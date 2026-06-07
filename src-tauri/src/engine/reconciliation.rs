use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSetBuilder};
use redb::Database;

use crate::engine::freshness::tracked_file_from_metadata;
use crate::engine::quiescence::is_transient_path;
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
    let mut observed_paths = HashSet::new();
    let mut report = ReconciliationReport::default();

    for target in config.watch_targets.iter().filter(|target| target.enabled) {
        let root = PathBuf::from(&target.path);
        if !root.exists() {
            continue;
        }
        let ignore_set = build_ignore_set(target)?;

        for path in scan_target_paths(&root, target.recursive)? {
            if is_transient_path(&path) {
                continue;
            }
            if target_ignores_path(target, &path, ignore_set.as_ref()) {
                mark_existing_ignored(db, &path, &mut report)?;
                continue;
            }
            if fs::symlink_metadata(&path)?.file_type().is_symlink() {
                continue;
            }
            let metadata = fs::metadata(&path)?;
            if !metadata.is_file() {
                continue;
            }

            let path_string = path.to_string_lossy().to_string();
            let existing = storage::tracked::get_tracked_file(db, &path_string)?;
            let target_config = config_for_target(&config, target.default_ttl_seconds);
            let mut tracked =
                tracked_file_from_metadata(&path, &metadata, existing.as_ref(), &target_config);
            tracked.matched_rule_ids = matching_rule_ids(&tracked, &config, &rules)?;
            match &existing {
                Some(existing) if tracked_file_changed(existing, &tracked) => {
                    report.updated.push(path_string.clone());
                }
                Some(_) => {}
                None => report.indexed.push(path_string.clone()),
            }
            storage::tracked::upsert_tracked_file(db, &tracked)?;
            observed_paths.insert(path_string.clone());
        }
    }

    for file in storage::tracked::list_tracked_files(db)? {
        if !observed_paths.contains(&file.path) {
            let path = Path::new(&file.path);
            let in_active_scope = config
                .watch_targets
                .iter()
                .filter(|target| target.enabled)
                .any(|target| root_contains(&target.path, path))
                || root_contains(&config.safe_folder_path, path);

            if !in_active_scope {
                if !matches!(file.state, FileDecayState::Missing) {
                    report.removed.push(file.path.clone());
                }
                storage::tracked::remove_tracked_file(db, &file.path)?;
            } else if !path.exists() {
                if !matches!(file.state, FileDecayState::Missing) {
                    report.removed.push(file.path.clone());
                }
                let mut updated_file = file;
                updated_file.state = FileDecayState::Missing;
                storage::tracked::upsert_tracked_file(db, &updated_file)?;
            }
        }
    }

    Ok(report)
}

fn scan_target_paths(root: &Path, recursive: bool) -> Result<Vec<PathBuf>, AppError> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() && recursive {
            paths.extend(scan_target_paths(&path, recursive)?);
        } else {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn config_for_target(config: &AppConfig, default_ttl_seconds: Option<u64>) -> AppConfig {
    let mut config = config.clone();
    if let Some(ttl) = default_ttl_seconds {
        config.default_ttl_seconds = ttl;
    }
    config
}

fn build_ignore_set(target: &WatchTarget) -> Result<Option<globset::GlobSet>, AppError> {
    if target.ignore_patterns.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in &target.ignore_patterns {
        builder.add(Glob::new(pattern).map_err(|error| {
            AppError::with_details(
                "RULE_INVALID_REGEX",
                "Watch target ignore pattern could not be parsed.",
                true,
                error.to_string(),
            )
        })?);
    }
    Ok(Some(builder.build().map_err(|error| {
        AppError::with_details(
            "RULE_INVALID_REGEX",
            "Watch target ignore pattern set could not be built.",
            true,
            error.to_string(),
        )
    })?))
}

fn mark_existing_ignored(
    db: &Database,
    path: &Path,
    report: &mut ReconciliationReport,
) -> Result<(), AppError> {
    let path_string = path.to_string_lossy().to_string();
    let Some(mut tracked) = storage::tracked::get_tracked_file(db, &path_string)? else {
        return Ok(());
    };
    if !matches!(tracked.state, FileDecayState::Ignored) {
        tracked.state = FileDecayState::Ignored;
        storage::tracked::upsert_tracked_file(db, &tracked)?;
        report.updated.push(path_string);
    }
    Ok(())
}

fn target_ignores_path(
    target: &WatchTarget,
    path: &Path,
    ignore_set: Option<&globset::GlobSet>,
) -> bool {
    let Some(ignore_set) = ignore_set else {
        return false;
    };
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let relative = Path::new(&target.path)
        .canonicalize()
        .ok()
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

fn root_contains(root: &str, path: &Path) -> bool {
    let Some(root) = normalize_configured_path(Path::new(root)) else {
        return false;
    };
    let Some(path) = normalize_configured_path(path) else {
        return false;
    };
    path.starts_with(root)
}

fn normalize_configured_path(path: &Path) -> Option<PathBuf> {
    use std::ffi::OsStr;
    let mut suffix = Vec::new();
    let mut cursor = path.to_path_buf();

    loop {
        if let Ok(canonical) = cursor.canonicalize() {
            let mut normalized = canonical;
            for component in suffix.iter().rev() {
                normalized.push(component);
            }
            return Some(normalized);
        }

        let component = cursor.file_name()?.to_os_string();
        if component == OsStr::new(".") || component == OsStr::new("..") {
            return None;
        }
        suffix.push(component);
        cursor = cursor.parent()?.to_path_buf();
    }
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

        assert_eq!(error.code, "RULE_INVALID_REGEX");
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

        fn save_config_with_ignore_patterns(&self, ignore_patterns: Vec<String>) {
            let config = AppConfig {
                watch_targets: vec![WatchTarget {
                    id: String::from("watch"),
                    path: path_string(&self.watch),
                    enabled: true,
                    recursive: false,
                    default_ttl_seconds: None,
                    ignore_patterns,
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

    fn path_string(path: &Path) -> String {
        path.to_string_lossy().to_string()
    }
}
