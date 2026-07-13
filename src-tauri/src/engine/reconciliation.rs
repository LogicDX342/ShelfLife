use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use rayon::prelude::*;
use std::sync::Mutex;

enum ObservedFile {
    Ignored(String),
    Present(Box<TrackedFile>),
}

#[derive(Clone, Copy)]
enum ExistingPathStatus {
    ActivePresent,
    ActiveMissing,
    Inactive,
}

struct ReconciliationPlan {
    report: ReconciliationReport,
    to_upsert: Vec<TrackedFile>,
    to_remove: Vec<String>,
}

struct ObservationContext<'a> {
    target: &'a WatchTarget,
    config: &'a AppConfig,
    rules: &'a CompiledRuleSet,
    now: u64,
    ignore_set: Option<&'a GlobSet>,
    canonical_root: Option<&'a Path>,
}

use crate::engine::paths::PathScope;
use crate::engine::quiescence::{is_hidden_directory, is_system_directory, is_transient_path};
use crate::engine::{project_watched_file, tracked_file_from_metadata};
use crate::models::{
    AppConfig, AppError, FileDecayState, ReconciliationReport, TrackedFile, WatchTarget,
};
use crate::rules::CompiledRuleSet;
use crate::storage::{self, Database};

pub fn reconcile_with_report_with_progress(
    db: &Database,
    progress_cb: Option<&(dyn Fn(usize, usize) + Send + Sync)>,
) -> Result<ReconciliationReport, AppError> {
    let config = storage::get_config(db)?;
    let scope = PathScope::new(&config);
    let rule_set = CompiledRuleSet::compile(storage::rules::list_rules(db)?, &config)?;
    let now = crate::engine::freshness::now_seconds();
    // Load all currently-tracked files into memory once (single read transaction).
    let existing_map: HashMap<String, TrackedFile> = storage::tracked::list_tracked_files(db)?
        .into_iter()
        .map(|f| (f.path.clone(), f))
        .collect();

    let mut observations = Vec::new();

    for target in config.watch_targets.iter().filter(|t| t.enabled) {
        let root = PathBuf::from(&target.path);
        if !root.exists() {
            continue;
        }

        // Hoist pattern set construction outside the file loop.
        let ignore_set = build_glob_set(&target.ignore_patterns)?;
        let hidden_whitelist = build_glob_set(&target.include_hidden_patterns)?;
        // Canonicalize root once — reused inside target_ignores_path.
        let canonical_root = root.canonicalize().ok();
        let paths = scan_target_paths(
            &root,
            target.recursive,
            ignore_set.as_ref(),
            hidden_whitelist.as_ref(),
        )?;
        let path_results: Result<Vec<Option<ObservedFile>>, AppError> = paths
            .into_par_iter()
            .map(|path| {
                let path_string = path.to_string_lossy().to_string();
                let context = ObservationContext {
                    target,
                    config: &config,
                    rules: &rule_set,
                    now,
                    ignore_set: ignore_set.as_ref(),
                    canonical_root: canonical_root.as_deref(),
                };
                observe_path(&path, existing_map.get(&path_string), &context)
            })
            .collect();

        observations.extend(path_results?.into_iter().flatten());
    }

    let path_statuses = path_statuses_for(&existing_map, &scope);
    reconcile_observations(db, &existing_map, observations, &path_statuses, progress_cb)
}

fn plan_reconciliation(
    existing: &HashMap<String, TrackedFile>,
    observations: Vec<ObservedFile>,
    path_statuses: &HashMap<String, ExistingPathStatus>,
) -> ReconciliationPlan {
    let mut observed_paths = HashSet::new();
    let mut report = ReconciliationReport::default();
    let mut to_upsert = Vec::new();

    for observation in observations {
        match observation {
            ObservedFile::Ignored(path) => {
                observed_paths.insert(path.clone());
                if let Some(file) = existing.get(&path) {
                    if !matches!(file.state, FileDecayState::Ignored) {
                        let mut updated = file.clone();
                        updated.state = FileDecayState::Ignored;
                        report.updated.push(path);
                        to_upsert.push(updated);
                    }
                }
            }
            ObservedFile::Present(tracked) => {
                observed_paths.insert(tracked.path.clone());
                match existing.get(&tracked.path) {
                    Some(file) if tracked_file_changed(file, &tracked) => {
                        report.updated.push(tracked.path.clone());
                    }
                    None => report.indexed.push(tracked.path.clone()),
                    _ => {}
                }
                to_upsert.push(*tracked);
            }
        }
    }

    let mut to_remove = Vec::new();
    for (path, file) in existing {
        if observed_paths.contains(path) {
            continue;
        }
        match path_statuses.get(path) {
            Some(ExistingPathStatus::Inactive) => {
                if !matches!(file.state, FileDecayState::Missing) {
                    report.removed.push(path.clone());
                }
                to_remove.push(path.clone());
            }
            Some(ExistingPathStatus::ActiveMissing) => {
                if !matches!(file.state, FileDecayState::Missing) {
                    report.removed.push(path.clone());
                    let mut updated = file.clone();
                    updated.state = FileDecayState::Missing;
                    to_upsert.push(updated);
                }
            }
            Some(ExistingPathStatus::ActivePresent) | None => {}
        }
    }

    ReconciliationPlan {
        report,
        to_upsert,
        to_remove,
    }
}

/// Incremental reconciliation for watcher events: processes only the given paths
/// instead of scanning the full watch tree. Used by the debounced event loop.
pub fn reconcile_paths(
    db: &Database,
    paths: Vec<PathBuf>,
) -> Result<ReconciliationReport, AppError> {
    let config = storage::get_config(db)?;
    let scope = PathScope::new(&config);
    let rule_set = CompiledRuleSet::compile(storage::rules::list_rules(db)?, &config)?;
    let now = crate::engine::freshness::now_seconds();

    // Deduplicate paths.
    let paths: Vec<PathBuf> = paths
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let mut existing_map = HashMap::new();
    for path in &paths {
        let path_string = path.to_string_lossy().to_string();
        if let Some(tracked) = storage::tracked::get_tracked_file(db, &path_string)? {
            existing_map.insert(path_string, tracked);
        }
    }

    let mut observations = Vec::new();

    for path in &paths {
        let path_string = path.to_string_lossy().to_string();

        // Find the matching watch target for scope validation.
        let Some(target) = scope.watch_target_for_path(path) else {
            continue;
        };

        let ignore_set = build_glob_set(&target.ignore_patterns)?;
        let canonical_root = PathBuf::from(&target.path).canonicalize().ok();

        let context = ObservationContext {
            target,
            config: &config,
            rules: &rule_set,
            now,
            ignore_set: ignore_set.as_ref(),
            canonical_root: canonical_root.as_deref(),
        };
        if let Some(observation) = observe_path(path, existing_map.get(&path_string), &context)? {
            observations.push(observation);
        }
    }

    let path_statuses = path_statuses_for(&existing_map, &scope);
    reconcile_observations(db, &existing_map, observations, &path_statuses, None)
}

fn observe_path(
    path: &Path,
    existing: Option<&TrackedFile>,
    context: &ObservationContext<'_>,
) -> Result<Option<ObservedFile>, AppError> {
    if is_transient_path(path) {
        return Ok(None);
    }

    let path_string = path.to_string_lossy().to_string();
    // Single stat — symlink_metadata covers both symlink detection and file metadata.
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(None),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(None);
    }
    if target_ignores_path(path, context.ignore_set, context.canonical_root) {
        return Ok(Some(ObservedFile::Ignored(path_string)));
    }

    let tracked = tracked_file_from_metadata(
        path,
        &metadata,
        existing,
        context.config,
        &context.target.id,
    );
    let tracked =
        project_watched_file(tracked, context.config, context.rules, context.now)?.tracked;

    Ok(Some(ObservedFile::Present(Box::new(tracked))))
}

fn path_statuses_for(
    existing: &HashMap<String, TrackedFile>,
    scope: &PathScope<'_>,
) -> HashMap<String, ExistingPathStatus> {
    existing
        .iter()
        .map(|(path_string, file)| {
            let path = Path::new(path_string);
            let status = if !scope.is_tracked_path_active(path, &file.watch_target_id) {
                ExistingPathStatus::Inactive
            } else if path.exists() {
                ExistingPathStatus::ActivePresent
            } else {
                ExistingPathStatus::ActiveMissing
            };
            (path_string.clone(), status)
        })
        .collect()
}

fn reconcile_observations(
    db: &Database,
    existing: &HashMap<String, TrackedFile>,
    observations: Vec<ObservedFile>,
    path_statuses: &HashMap<String, ExistingPathStatus>,
    progress_cb: Option<&(dyn Fn(usize, usize) + Send + Sync)>,
) -> Result<ReconciliationReport, AppError> {
    let plan = plan_reconciliation(existing, observations, path_statuses);
    let ReconciliationPlan {
        report,
        to_upsert,
        to_remove,
    } = plan;

    if let Some(cb) = progress_cb {
        let total_changes = to_upsert.len() + to_remove.len();
        let last_emit = Mutex::new(std::time::Instant::now());
        let progress_emitter = |current| {
            let is_first = current == 1;
            let is_last = current == total_changes;
            let should_emit = if is_first || is_last {
                true
            } else {
                let mut last = last_emit.lock().unwrap();
                if last.elapsed() >= std::time::Duration::from_millis(100) {
                    *last = std::time::Instant::now();
                    true
                } else {
                    false
                }
            };

            if should_emit {
                cb(current, total_changes);
            }
        };
        storage::tracked::update_tracked_files_batch_with_progress(
            db,
            to_upsert,
            to_remove,
            Some(&progress_emitter),
        )?;
    } else {
        storage::tracked::update_tracked_files_batch(db, to_upsert, to_remove)?;
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
    use std::sync::{Arc, Mutex};

    use uuid::Uuid;

    use crate::models::{
        AppConfig, AppError, Expiry, FileDecayState, OriginEvidence, TrackedFile, WatchTarget,
    };
    use crate::storage::{self, Database};

    use super::{
        plan_reconciliation, reconcile_paths, reconcile_with_report_with_progress,
        ExistingPathStatus, ObservedFile,
    };

    fn tracked(path: &str) -> TrackedFile {
        TrackedFile {
            path: path.into(),
            file_name: path.into(),
            watch_target_id: "watch".into(),
            size_bytes: 1,
            first_seen_at: 1,
            last_observed_mtime: Some(1),
            last_observed_atime: None,
            last_user_action_at: None,
            freshness_at: 1,
            expiry: Expiry::At(2),
            state: FileDecayState::Fresh,
            matched_rule_ids: Vec::new(),
            origin: OriginEvidence::Unknown,
        }
    }

    #[test]
    fn planner_classifies_indexed_updated_and_missing_files_without_storage() {
        let unchanged = tracked("unchanged");
        let mut updated = tracked("updated");
        updated.size_bytes = 2;
        let missing = tracked("missing");
        let existing = [
            (unchanged.path.clone(), unchanged.clone()),
            (updated.path.clone(), tracked("updated")),
            (missing.path.clone(), missing),
        ]
        .into_iter()
        .collect();
        let statuses = [("missing".into(), ExistingPathStatus::ActiveMissing)]
            .into_iter()
            .collect();

        let plan = plan_reconciliation(
            &existing,
            vec![
                ObservedFile::Present(Box::new(unchanged)),
                ObservedFile::Present(Box::new(updated)),
                ObservedFile::Present(Box::new(tracked("indexed"))),
            ],
            &statuses,
        );

        assert_eq!(plan.report.indexed, vec!["indexed"]);
        assert_eq!(plan.report.updated, vec!["updated"]);
        assert_eq!(plan.report.removed, vec!["missing"]);
        assert!(plan
            .to_upsert
            .iter()
            .any(|file| file.path == "missing" && file.state == FileDecayState::Missing));
    }

    #[test]
    fn planner_removes_inactive_files_and_marks_ignored_observations() {
        let inactive = tracked("inactive");
        let ignored = tracked("ignored");
        let existing = [
            (inactive.path.clone(), inactive),
            (ignored.path.clone(), ignored),
        ]
        .into_iter()
        .collect();
        let statuses = [("inactive".into(), ExistingPathStatus::Inactive)]
            .into_iter()
            .collect();

        let plan = plan_reconciliation(
            &existing,
            vec![ObservedFile::Ignored("ignored".into())],
            &statuses,
        );

        assert_eq!(plan.to_remove, vec!["inactive"]);
        assert_eq!(plan.report.removed, vec!["inactive"]);
        assert_eq!(plan.report.updated, vec!["ignored"]);
        assert!(plan
            .to_upsert
            .iter()
            .any(|file| file.path == "ignored" && file.state == FileDecayState::Ignored));
    }

    fn reconcile(db: &Database) -> Result<Vec<String>, AppError> {
        Ok(reconcile_with_report_with_progress(db, None)?.indexed)
    }

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
    fn reconcile_reports_processed_file_progress() {
        let fixture = Fixture::new();
        fixture.write_watch_file("first.txt", "first");
        fixture.write_watch_file("second.txt", "second");
        fixture.save_config();

        let progress = Arc::new(Mutex::new(Vec::new()));
        let progress_for_callback = Arc::clone(&progress);
        let processing_progress = move |current, total| {
            progress_for_callback
                .lock()
                .expect("progress lock should work")
                .push((current, total));
        };

        reconcile_with_report_with_progress(&fixture.db, Some(&processing_progress))
            .expect("reconciliation should succeed");

        let progress = progress.lock().expect("progress lock should work");
        assert_eq!(progress.last(), Some(&(2, 2)));
        assert!(progress.windows(2).all(|window| window[0].0 < window[1].0));
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

        let report = reconcile_with_report_with_progress(&fixture.db, None)
            .expect("reconciliation should succeed");
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
        let report = reconcile_with_report_with_progress(&fixture.db, None)
            .expect("reconciliation should succeed");
        let tracked = storage::tracked::list_tracked_files(&fixture.db)
            .expect("tracked list should work")
            .into_iter()
            .find(|tracked| tracked.file_name == "skip.me")
            .expect("tracked file should exist");

        assert_eq!(tracked.state, FileDecayState::Ignored);
        assert_eq!(report.updated, vec![path_string(&file)]);
    }

    #[test]
    fn incremental_reconciliation_marks_existing_file_ignored() {
        let fixture = Fixture::new();
        let file = fixture.write_watch_file("skip.me", "ignored later");
        fixture.save_config();
        reconcile(&fixture.db).expect("initial reconciliation should succeed");

        fixture.save_config_with_ignore_patterns(vec![String::from("*.me")]);
        let report = reconcile_paths(&fixture.db, vec![file.clone()])
            .expect("incremental reconciliation should succeed");
        let tracked = storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
            .expect("tracked lookup should work")
            .expect("tracked file should exist");

        assert_eq!(tracked.state, FileDecayState::Ignored);
        assert_eq!(report.updated, vec![path_string(&file)]);
    }

    #[test]
    fn incremental_reconciliation_marks_deleted_ignored_file_missing() {
        let fixture = Fixture::new();
        let file = fixture.write_watch_file("skip.me", "ignored later");
        fixture.save_config();
        reconcile(&fixture.db).expect("initial reconciliation should succeed");

        fixture.save_config_with_ignore_patterns(vec![String::from("*.me")]);
        reconcile_paths(&fixture.db, vec![file.clone()])
            .expect("incremental ignore reconciliation should succeed");
        fs::remove_file(&file).expect("test file should be removable");

        let report = reconcile_paths(&fixture.db, vec![file.clone()])
            .expect("incremental deletion reconciliation should succeed");
        let tracked = storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
            .expect("tracked lookup should work")
            .expect("tracked file should remain for missing state");

        assert_eq!(tracked.state, FileDecayState::Missing);
        assert_eq!(report.removed, vec![path_string(&file)]);
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

    struct Fixture {
        root: PathBuf,
        watch: PathBuf,
        db: Database,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("shelflife-reconcile-{}", Uuid::new_v4()));
            let watch = root.join("watch");
            fs::create_dir_all(&watch).expect("watch directory should be created");
            let db =
                storage::open_database(root.join("test.sqlite")).expect("database should open");
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
                    ignore_patterns: Vec::new(),
                    include_hidden_patterns: Vec::new(),
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
                    ignore_patterns,
                    include_hidden_patterns: Vec::new(),
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
    fn reconcile_scales_rule_ttl_decay_window_from_global_ratio() {
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

        let mut config = storage::get_config(&fixture.db).expect("config should load");
        config.decaying_threshold_seconds = 48 * 60 * 60; // 48h warning buffer
        storage::save_config(&fixture.db, &config).expect("config should save");

        reconcile(&fixture.db).expect("reconciliation should succeed");

        let tracked = storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
            .expect("tracked lookup should work")
            .expect("tracked file should exist");

        assert_eq!(tracked.matched_rule_ids, vec![String::from("zip-rule")]);
        assert_eq!(
            tracked.expiry,
            crate::models::Expiry::At(tracked.freshness_at + 24 * 60 * 60)
        );
        assert_eq!(tracked.state, FileDecayState::Fresh);
    }

    #[test]
    fn reconcile_treats_ignore_rule_as_filter_without_rule_ttl() {
        let fixture = Fixture::new();
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.save_config();

        let rule = crate::models::AutomationRule {
            id: String::from("zip-ignore-rule"),
            name: String::from("Ignore zip downloads"),
            enabled: true,
            priority: 10,
            watch_path: path_string(&fixture.watch),
            ttl_seconds: 1,
            conditions: crate::models::RuleConditions {
                extensions: vec![String::from("zip")],
                filename_globs: Vec::new(),
                filename_regexes: Vec::new(),
                source_domains: Vec::new(),
                size: crate::models::SizeCondition::Any,
            },
            action: crate::models::RuleAction::Ignore,
            mode: crate::models::RuleMode::Automatic,
            created_at: 1,
            updated_at: 1,
        };
        storage::rules::save_rule(&fixture.db, &rule).expect("rule should save");

        reconcile(&fixture.db).expect("reconciliation should succeed");

        let tracked = storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
            .expect("tracked lookup should work")
            .expect("tracked file should exist");

        assert_eq!(tracked.state, FileDecayState::Ignored);
        assert_eq!(
            tracked.expiry,
            crate::models::Expiry::At(
                tracked.freshness_at + crate::models::AppConfig::default().default_ttl_seconds
            )
        );
    }

    #[test]
    fn reconcile_removes_subfolder_files_when_switched_to_non_recursive() {
        let fixture = Fixture::new();
        // Create files in root and subfolder
        let root_file = fixture.write_watch_file("root.txt", "root");
        let sub_dir = fixture.watch.join("sub");
        fs::create_dir_all(&sub_dir).expect("sub directory should be created");
        let sub_file = sub_dir.join("nested.txt");
        fs::write(&sub_file, "nested").expect("nested file should be written");

        // Index in recursive mode
        fixture.save_config_recursive();
        reconcile(&fixture.db).expect("recursive reconciliation should succeed");

        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&root_file))
                .expect("tracked lookup should work")
                .is_some(),
            "root file should be tracked in recursive mode"
        );
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&sub_file))
                .expect("tracked lookup should work")
                .is_some(),
            "nested file should be tracked in recursive mode"
        );

        // Switch to non-recursive (top-level only)
        fixture.save_config();
        let report = reconcile_with_report_with_progress(&fixture.db, None)
            .expect("non-recursive reconciliation should succeed");

        // Root file stays, subfolder file is removed
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&root_file))
                .expect("tracked lookup should work")
                .is_some(),
            "root file should remain tracked in top-level mode"
        );
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&sub_file))
                .expect("tracked lookup should work")
                .is_none(),
            "nested file should be removed in top-level mode"
        );
        assert!(
            report.removed.contains(&path_string(&sub_file)),
            "nested file should appear in removed report"
        );
    }

    #[test]
    fn reconcile_sets_and_uses_watch_target_id() {
        let fixture = Fixture::new();
        let file = fixture.write_watch_file("download.txt", "body");
        fixture.save_config();

        reconcile(&fixture.db).expect("reconciliation should succeed");

        let tracked = storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
            .expect("tracked lookup should work")
            .expect("tracked file should exist");

        assert_eq!(tracked.watch_target_id, "watch");
    }

    fn path_string(path: &Path) -> String {
        path.to_string_lossy().to_string()
    }
}
