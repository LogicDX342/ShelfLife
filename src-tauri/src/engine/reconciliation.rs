use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use redb::Database;

use crate::engine::freshness::tracked_file_from_metadata;
use crate::engine::paths::root_contains;
use crate::engine::quiescence::{is_hidden_directory, is_system_directory, is_transient_path};
use crate::models::{
    AppConfig, AppError, FileDecayState, ReconciliationReport, TrackedFile, WatchTarget,
};
use crate::rules::matching_rule_ids;
use crate::storage;

pub fn reconcile(db: &Database) -> Result<Vec<String>, AppError> {
    Ok(reconcile_with_report(db)?.indexed)
}

pub fn reconcile_with_report(db: &Database) -> Result<ReconciliationReport, AppError> {
    let config = storage::get_config(db)?;
    let rules = storage::rules::list_rules(db)?;
    let mut observed_paths: HashSet<String> = HashSet::new();
    let mut report = ReconciliationReport::default();

    // Load all currently-tracked files into memory once (single read transaction).
    let existing_map: HashMap<String, TrackedFile> = storage::tracked::list_tracked_files(db)?
        .into_iter()
        .map(|f| (f.path.clone(), f))
        .collect();

    let mut to_upsert: Vec<TrackedFile> = Vec::new();
    let mut to_mark_ignored: Vec<String> = Vec::new();

    for target in config.watch_targets.iter().filter(|t| t.enabled) {
        let root = PathBuf::from(&target.path);
        if !root.exists() {
            continue;
        }

        // Hoist pattern set construction outside the file loop.
        let ignore_set = build_ignore_set(target)?;
        let hidden_whitelist = build_hidden_whitelist(target)?;
        // Canonicalize root once — reused inside target_ignores_path.
        let canonical_root = root.canonicalize().ok();
        let effective_ttl_seconds = effective_ttl_seconds(&config, target);

        for path in scan_target_paths(
            &root,
            target.recursive,
            ignore_set.as_ref(),
            hidden_whitelist.as_ref(),
        )? {
            if is_transient_path(&path) {
                continue;
            }
            if target_ignores_path(&path, ignore_set.as_ref(), canonical_root.as_deref()) {
                // File-level ignore pattern matched — mark existing as Ignored.
                let path_string = path.to_string_lossy().to_string();
                if let Some(existing) = existing_map.get(&path_string) {
                    if !matches!(existing.state, FileDecayState::Ignored) {
                        to_mark_ignored.push(path_string.clone());
                        report.updated.push(path_string);
                    }
                }
                continue;
            }

            // Single stat — symlink_metadata covers both symlink detection and file metadata.
            let metadata = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                continue;
            }

            let path_string = path.to_string_lossy().to_string();
            observed_paths.insert(path_string.clone());

            let existing = existing_map.get(&path_string);
            let mut tracked = tracked_file_from_metadata(
                &path,
                &metadata,
                existing,
                &config,
                effective_ttl_seconds,
            );

            // Always run rule matching to ensure we match new/deleted/modified rules
            tracked.matched_rule_ids = matching_rule_ids(&tracked, &config, &rules)?;

            // Apply rules to adjust expiry & state
            crate::engine::freshness::apply_rules_to_tracked_file(
                &mut tracked,
                &rules,
                &config,
                effective_ttl_seconds,
                crate::engine::freshness::now_seconds(),
            );

            match existing {
                Some(e) if tracked_file_changed(e, &tracked) => {
                    report.updated.push(path_string);
                }
                None => {
                    report.indexed.push(path_string);
                }
                _ => {}
            }
            to_upsert.push(tracked);
        }
    }

    // Apply Ignored state updates gathered during the scan.
    for path_string in to_mark_ignored {
        if let Some(existing) = existing_map.get(&path_string) {
            let mut updated = existing.clone();
            updated.state = FileDecayState::Ignored;
            to_upsert.push(updated);
        }
    }

    // Handle previously-tracked files that were not observed in this scan.
    let mut to_remove: Vec<String> = Vec::new();
    let mut to_mark_missing: Vec<TrackedFile> = Vec::new();

    for (path_string, file) in &existing_map {
        if observed_paths.contains(path_string) {
            continue;
        }
        let path = Path::new(path_string);
        let in_active_scope = config
            .watch_targets
            .iter()
            .filter(|t| t.enabled)
            .any(|t| root_contains(&t.path, path))
            || root_contains(&config.safe_folder_path, path);

        if !in_active_scope {
            if !matches!(file.state, FileDecayState::Missing) {
                report.removed.push(path_string.clone());
            }
            to_remove.push(path_string.clone());
        } else if !path.exists() {
            if !matches!(file.state, FileDecayState::Missing) {
                report.removed.push(path_string.clone());
            }
            let mut updated = file.clone();
            updated.state = FileDecayState::Missing;
            to_mark_missing.push(updated);
        }
    }

    to_upsert.extend(to_mark_missing);

    // Single batch write for all upserts + one batch remove.
    if !to_upsert.is_empty() {
        storage::tracked::upsert_tracked_files_batch(db, &to_upsert)?;
    }
    if !to_remove.is_empty() {
        let refs: Vec<&str> = to_remove.iter().map(|s| s.as_str()).collect();
        storage::tracked::remove_tracked_files_batch(db, &refs)?;
    }

    Ok(report)
}

/// Incremental reconciliation for watcher events: processes only the given paths
/// instead of scanning the full watch tree. Used by the debounced event loop.
pub fn reconcile_paths(
    db: &Database,
    paths: Vec<PathBuf>,
) -> Result<ReconciliationReport, AppError> {
    let config = storage::get_config(db)?;
    let rules = storage::rules::list_rules(db)?;
    let mut report = ReconciliationReport::default();
    let mut to_upsert: Vec<TrackedFile> = Vec::new();

    // Deduplicate paths.
    let paths: Vec<PathBuf> = paths
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    for path in &paths {
        if is_transient_path(path) {
            continue;
        }

        let path_string = path.to_string_lossy().to_string();

        // Find the matching watch target for scope validation.
        let target = config
            .watch_targets
            .iter()
            .filter(|t| t.enabled)
            .find(|t| root_contains(&t.path, path));

        // File deleted (or outside scope) — mark Missing if we're tracking it.
        if !path.exists() {
            if let Some(mut tracked) = storage::tracked::get_tracked_file(db, &path_string)? {
                if !matches!(tracked.state, FileDecayState::Missing) {
                    report.removed.push(path_string.clone());
                    tracked.state = FileDecayState::Missing;
                    to_upsert.push(tracked);
                }
            }
            continue;
        }

        // Ignore paths outside any enabled watch target scope.
        let Some(target) = target else {
            continue;
        };

        let ignore_set = build_ignore_set(target)?;
        let canonical_root = PathBuf::from(&target.path).canonicalize().ok();

        if target_ignores_path(path, ignore_set.as_ref(), canonical_root.as_deref()) {
            continue;
        }

        let metadata = match fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            continue;
        }

        let existing = storage::tracked::get_tracked_file(db, &path_string)?;
        let effective_ttl_seconds = effective_ttl_seconds(&config, target);
        let mut tracked = tracked_file_from_metadata(
            path,
            &metadata,
            existing.as_ref(),
            &config,
            effective_ttl_seconds,
        );

        // Always run rule matching to ensure we match new/deleted/modified rules
        tracked.matched_rule_ids = matching_rule_ids(&tracked, &config, &rules)?;

        // Apply rules to adjust expiry & state
        crate::engine::freshness::apply_rules_to_tracked_file(
            &mut tracked,
            &rules,
            &config,
            effective_ttl_seconds,
            crate::engine::freshness::now_seconds(),
        );

        match &existing {
            Some(e) if tracked_file_changed(e, &tracked) => {
                report.updated.push(path_string);
            }
            None => {
                report.indexed.push(path_string);
            }
            _ => {}
        }
        to_upsert.push(tracked);
    }

    if !to_upsert.is_empty() {
        storage::tracked::upsert_tracked_files_batch(db, &to_upsert)?;
    }

    Ok(report)
}

fn scan_target_paths(
    root: &Path,
    recursive: bool,
    ignore_set: Option<&GlobSet>,
    hidden_whitelist: Option<&GlobSet>,
) -> Result<Vec<PathBuf>, AppError> {
    scan_target_paths_inner(root, recursive, true, ignore_set, hidden_whitelist)
}

fn scan_target_paths_inner(
    root: &Path,
    recursive: bool,
    is_root: bool,
    ignore_set: Option<&GlobSet>,
    hidden_whitelist: Option<&GlobSet>,
) -> Result<Vec<PathBuf>, AppError> {
    let mut paths = Vec::new();
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => {
            if is_root {
                return Err(error.into());
            } else {
                return Ok(paths);
            }
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(error) => {
                if is_root {
                    return Err(error.into());
                } else {
                    continue;
                }
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(error) => {
                if is_root {
                    return Err(error.into());
                } else {
                    continue;
                }
            }
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() && recursive {
            // System directories are always skipped — no override.
            if is_system_directory(&path) {
                continue;
            }
            // Hidden directories are skipped by default; allowed if whitelisted.
            if is_hidden_directory(&path) {
                let dir_name = path
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or_default();
                let whitelisted = hidden_whitelist
                    .map(|set| set.is_match(dir_name))
                    .unwrap_or(false);
                if !whitelisted {
                    continue;
                }
            }
            // Non-hidden dirs can be excluded by ignore_patterns.
            if let Some(set) = ignore_set {
                let dir_name = path
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or_default();
                if set.is_match(dir_name) {
                    continue;
                }
            }
            if let Ok(sub_paths) =
                scan_target_paths_inner(&path, recursive, false, ignore_set, hidden_whitelist)
            {
                paths.extend(sub_paths);
            }
        } else {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn effective_ttl_seconds(config: &AppConfig, target: &WatchTarget) -> u64 {
    target
        .default_ttl_seconds
        .unwrap_or(config.default_ttl_seconds)
}

fn build_ignore_set(target: &WatchTarget) -> Result<Option<GlobSet>, AppError> {
    build_glob_set(&target.ignore_patterns)
}

fn build_hidden_whitelist(target: &WatchTarget) -> Result<Option<GlobSet>, AppError> {
    build_glob_set(&target.include_hidden_patterns)
}

fn build_glob_set(patterns: &[String]) -> Result<Option<GlobSet>, AppError> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern).map_err(|error| {
            AppError::with_details(
                "RULE_INVALID_GLOB",
                "Watch target pattern could not be parsed.",
                true,
                error.to_string(),
            )
        })?);
    }
    Ok(Some(builder.build().map_err(|error| {
        AppError::with_details(
            "RULE_INVALID_GLOB",
            "Watch target pattern set could not be built.",
            true,
            error.to_string(),
        )
    })?))
}

fn target_ignores_path(
    path: &Path,
    ignore_set: Option<&GlobSet>,
    canonical_root: Option<&Path>,
) -> bool {
    let Some(ignore_set) = ignore_set else {
        return false;
    };
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    // Use the pre-canonicalized root to avoid a per-file syscall.
    let relative = canonical_root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);
    ignore_set.is_match(file_name) || ignore_set.is_match(relative)
}

fn tracked_file_changed(existing: &TrackedFile, next: &TrackedFile) -> bool {
    existing.file_name != next.file_name
        || existing.size_bytes != next.size_bytes
        || existing.last_observed_mtime != next.last_observed_mtime
        || existing.freshness_at != next.freshness_at
        || existing.expiry != next.expiry
        || existing.state != next.state
        || existing.matched_rule_ids != next.matched_rule_ids
        || existing.origin != next.origin
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use uuid::Uuid;

    use crate::models::{AppConfig, FileDecayState, WatchTarget};
    use crate::storage;

    use super::{reconcile, reconcile_with_report};

    #[test]
    fn reconcile_indexes_existing_files_and_marks_missing_rows() {
        let fixture = Fixture::new();
        let file = fixture.write_watch_file("download.txt", "body");
        fixture.save_config();

        let indexed = reconcile(&fixture.db).expect("reconciliation should succeed");
        assert_eq!(indexed, vec![path_string(&file)]);
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
                .expect("tracked lookup should work")
                .is_some()
        );

        fs::remove_file(&file).expect("test file should be removable");
        reconcile(&fixture.db).expect("second reconciliation should succeed");
        let tracked = storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
            .expect("tracked lookup should work")
            .expect("tracked row should remain for missing state");
        assert_eq!(tracked.state, FileDecayState::Missing);
    }

    #[test]
    fn reconcile_removes_files_outside_watch_targets() {
        let fixture = Fixture::new();
        let file = fixture.write_watch_file("download.txt", "body");
        fixture.save_config();

        let indexed = reconcile(&fixture.db).expect("reconciliation should succeed");
        assert_eq!(indexed, vec![path_string(&file)]);
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
                .expect("tracked lookup should work")
                .is_some()
        );

        let config = AppConfig {
            watch_targets: Vec::new(),
            safe_folder_path: path_string(&fixture.root.join("safe")),
            ..AppConfig::default()
        };
        storage::save_config(&fixture.db, &config).expect("config should save");

        let report = reconcile_with_report(&fixture.db).expect("reconciliation should succeed");
        assert_eq!(report.removed, vec![path_string(&file)]);
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
                .expect("tracked lookup should work")
                .is_none()
        );
    }

    #[test]
    fn reconcile_ignores_transient_partial_files() {
        let fixture = Fixture::new();
        let partial = fixture.write_watch_file("asset.zip.crdownload", "partial");
        fixture.save_config();

        let indexed = reconcile(&fixture.db).expect("reconciliation should succeed");
        assert!(indexed.is_empty());
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&partial))
                .expect("tracked lookup should work")
                .is_none()
        );
    }

    #[test]
    fn reconcile_applies_watch_target_ignore_patterns_before_indexing() {
        let fixture = Fixture::new();
        let ignored = fixture.write_watch_file("skip.me", "ignored");
        let kept = fixture.write_watch_file("keep.txt", "kept");
        fixture.save_config_with_ignore_patterns(vec![String::from("*.me")]);

        let indexed = reconcile(&fixture.db).expect("reconciliation should succeed");

        assert_eq!(indexed, vec![path_string(&kept)]);
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&ignored))
                .expect("tracked lookup should work")
                .is_none()
        );
    }

    #[test]
    fn reconcile_marks_existing_file_ignored_when_ignore_pattern_is_added() {
        let fixture = Fixture::new();
        let file = fixture.write_watch_file("skip.me", "ignored later");
        fixture.save_config();
        reconcile(&fixture.db).expect("initial reconciliation should succeed");
        assert!(storage::tracked::list_tracked_files(&fixture.db)
            .expect("tracked list should work")
            .iter()
            .any(|tracked| tracked.file_name == "skip.me"));

        fixture.save_config_with_ignore_patterns(vec![String::from("*.me")]);
        let report = reconcile_with_report(&fixture.db).expect("reconciliation should succeed");
        let tracked = storage::tracked::list_tracked_files(&fixture.db)
            .expect("tracked list should work")
            .into_iter()
            .find(|tracked| tracked.file_name == "skip.me")
            .expect("tracked file should exist");

        assert_eq!(tracked.state, FileDecayState::Ignored);
        assert_eq!(report.updated, vec![path_string(&file)]);
    }

    #[test]
    fn invalid_watch_target_ignore_pattern_is_rejected() {
        let fixture = Fixture::new();
        fixture.write_watch_file("file.txt", "body");
        fixture.save_config_with_ignore_patterns(vec![String::from("[")]);

        let error = reconcile(&fixture.db).expect_err("invalid ignore pattern should fail");

        assert_eq!(error.code, "RULE_INVALID_GLOB");
    }

    #[test]
    fn reconcile_skips_system_subdirectory_during_recursive_scan() {
        let fixture = Fixture::new();
        let system_dir = fixture.watch.join("$RECYCLE.BIN");
        fs::create_dir_all(&system_dir).expect("system dir should be created");
        let system_file = system_dir.join("deleted.txt");
        fs::write(&system_file, "trash").expect("system file should be written");
        fixture.write_watch_file("keep.txt", "kept");
        fixture.save_config_recursive();

        let indexed = reconcile(&fixture.db).expect("reconciliation should succeed");

        // Only the non-system file should be indexed.
        assert_eq!(indexed.len(), 1);
        assert!(indexed[0].ends_with("keep.txt"));
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&system_file))
                .expect("tracked lookup should work")
                .is_none()
        );
    }

    #[cfg(unix)]
    #[test]
    fn reconcile_skips_symlink_files() {
        let fixture = Fixture::new();
        let outside = fixture.root.join("outside.txt");
        fs::write(&outside, "outside").expect("outside file should be written");
        let link = fixture.watch.join("outside-link.txt");
        std::os::unix::fs::symlink(&outside, &link).expect("symlink should be created");
        fixture.save_config();

        let indexed = reconcile(&fixture.db).expect("reconciliation should succeed");

        assert!(indexed.is_empty());
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&link))
                .expect("tracked lookup should work")
                .is_none()
        );
    }

    #[cfg(windows)]
    #[test]
    fn reconcile_skips_symlink_files() {
        let fixture = Fixture::new();
        let outside = fixture.root.join("outside.txt");
        fs::write(&outside, "outside").expect("outside file should be written");
        let link = fixture.watch.join("outside-link.txt");
        if std::os::windows::fs::symlink_file(&outside, &link).is_err() {
            return;
        }
        fixture.save_config();

        let indexed = reconcile(&fixture.db).expect("reconciliation should succeed");

        assert!(indexed.is_empty());
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&link))
                .expect("tracked lookup should work")
                .is_none()
        );
    }

    #[test]
    fn reconcile_report_separates_indexed_updated_and_removed_paths() {
        let fixture = Fixture::new();
        let updated = fixture.write_watch_file("updated.txt", "old");
        let removed = fixture.write_watch_file("removed.txt", "gone");
        fixture.save_config();
        reconcile(&fixture.db).expect("initial reconciliation should succeed");

        let indexed = fixture.write_watch_file("new.txt", "new");
        fs::write(&updated, "new content").expect("tracked file should be updated");
        fs::remove_file(&removed).expect("tracked file should be removed");

        let report = reconcile_with_report(&fixture.db).expect("report should build");

        assert_eq!(report.indexed, vec![path_string(&indexed)]);
        assert_eq!(report.updated, vec![path_string(&updated)]);
        assert_eq!(report.removed, vec![path_string(&removed)]);
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&indexed))
                .expect("tracked lookup should work")
                .is_some()
        );
    }

    struct Fixture {
        root: PathBuf,
        watch: PathBuf,
        db: std::sync::Arc<redb::Database>,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("shelflife-reconcile-{}", Uuid::new_v4()));
            let watch = root.join("watch");
            fs::create_dir_all(&watch).expect("watch directory should be created");
            let db = storage::open_database(root.join("test.redb")).expect("database should open");
            Self { root, watch, db }
        }

        fn save_config(&self) {
            self.save_config_with_ignore_patterns(Vec::new());
        }

        fn save_config_recursive(&self) {
            let config = AppConfig {
                watch_targets: vec![WatchTarget {
                    id: String::from("watch"),
                    path: path_string(&self.watch),
                    enabled: true,
                    recursive: true,
                    default_ttl_seconds: None,
                    ignore_patterns: Vec::new(),
                    include_hidden_patterns: Vec::new(),
                    rule_ids: Vec::new(),
                }],
                safe_folder_path: path_string(&self.root.join("safe")),
                ..AppConfig::default()
            };
            storage::save_config(&self.db, &config).expect("config should save");
        }

        fn save_config_with_ignore_patterns(&self, ignore_patterns: Vec<String>) {
            let config = AppConfig {
                watch_targets: vec![WatchTarget {
                    id: String::from("watch"),
                    path: path_string(&self.watch),
                    enabled: true,
                    recursive: false,
                    default_ttl_seconds: None,
                    ignore_patterns,
                    include_hidden_patterns: Vec::new(),
                    rule_ids: Vec::new(),
                }],
                safe_folder_path: path_string(&self.root.join("safe")),
                ..AppConfig::default()
            };
            storage::save_config(&self.db, &config).expect("config should save");
        }

        fn write_watch_file(&self, name: &str, content: &str) -> PathBuf {
            let path = self.watch.join(name);
            fs::write(&path, content).expect("test file should be written");
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn reconcile_applies_rule_ttl_to_decay_state() {
        let fixture = Fixture::new();
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.save_config();

        // 1. Save an enabled rule matching "zip" extension with 1 day TTL
        let rule = crate::models::AutomationRule {
            id: String::from("zip-rule"),
            name: String::from("Zip downloads"),
            enabled: true,
            priority: 10,
            watch_path: path_string(&fixture.watch),
            ttl_seconds: 24 * 60 * 60, // 1 day
            conditions: crate::models::RuleConditions {
                extensions: vec![String::from("zip")],
                filename_globs: Vec::new(),
                filename_regexes: Vec::new(),
                source_domains: Vec::new(),
                size: crate::models::SizeCondition::Any,
            },
            action: crate::models::RuleAction::Trash,
            mode: crate::models::RuleMode::Automatic,
            created_at: 1,
            updated_at: 1,
        };
        storage::rules::save_rule(&fixture.db, &rule).expect("rule should save");

        // Set config decay thresholds: buffer = 48 hours
        let mut config = storage::get_config(&fixture.db).expect("config should load");
        config.decaying_threshold_seconds = 48 * 60 * 60; // 48h warning buffer
        storage::save_config(&fixture.db, &config).expect("config should save");

        // Run reconciliation
        reconcile(&fixture.db).expect("reconciliation should succeed");

        // Verify the file state is Decaying and expiry is 1 day from now/freshness_at
        let tracked = storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
            .expect("tracked lookup should work")
            .expect("tracked file should exist");

        assert_eq!(tracked.matched_rule_ids, vec![String::from("zip-rule")]);
        assert_eq!(
            tracked.expiry,
            crate::models::Expiry::At(tracked.freshness_at + 24 * 60 * 60)
        );
        assert_eq!(tracked.state, FileDecayState::Decaying);
    }

    fn path_string(path: &Path) -> String {
        path.to_string_lossy().to_string()
    }
}
