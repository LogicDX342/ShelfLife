use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use rayon::prelude::*;
use std::sync::Mutex;

struct ReconciliationPlan {
    report: ReconciliationReport,
    to_insert: Vec<TrackedFile>,
    to_update: Vec<TrackedFile>,
    to_remove: Vec<String>,
}

pub struct IncrementalReconciliationOutcome {
    pub report: ReconciliationReport,
    pub arrival_candidates: Vec<AutomaticRuleCandidate>,
}

struct ScannedFile {
    path: PathBuf,
    metadata: fs::Metadata,
}

struct ObservationContext<'a> {
    target: &'a WatchTarget,
    config: &'a AppConfig,
    rules: &'a CompiledRuleSet,
    now: u64,
}

use crate::engine::paths::PathScope;
use crate::engine::quiescence::{is_hidden_path, is_system_directory, is_transient_path};
use crate::engine::{
    arrival_rule_candidate, project_watched_file, tracked_file_from_metadata,
    AutomaticRuleCandidate, TrackedRuleProjection,
};
use crate::models::{AppConfig, AppError, ReconciliationReport, TrackedFile, WatchTarget};
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
    let mut excluded_paths = HashSet::new();

    for target in config.watch_targets.iter().filter(|t| t.enabled) {
        let root = PathBuf::from(&target.path);
        if !root.exists() {
            continue;
        }

        // Hoist pattern set construction outside the file loop.
        let ignore_set = build_glob_set(&target.ignore_patterns)?;
        // Canonicalize root once — reused inside target_ignores_path.
        let canonical_root = root.canonicalize().ok();
        excluded_paths.extend(
            existing_map
                .values()
                .filter(|file| file.watch_target_id == target.id)
                .filter(|file| {
                    target_ignores_path(
                        Path::new(&file.path),
                        ignore_set.as_ref(),
                        &root,
                        canonical_root.as_deref(),
                    )
                })
                .map(|file| file.path.clone()),
        );
        let files = scan_target_paths(&root, target.recursive, ignore_set.as_ref())?
            .into_iter()
            .filter(|file| {
                !target_ignores_path(
                    &file.path,
                    ignore_set.as_ref(),
                    &root,
                    canonical_root.as_deref(),
                )
            })
            .collect::<Vec<_>>();
        let path_results: Result<Vec<Option<TrackedFile>>, AppError> = files
            .into_par_iter()
            .map(|file| {
                let path_string = file.path.to_string_lossy().to_string();
                let context = ObservationContext {
                    target,
                    config: &config,
                    rules: &rule_set,
                    now,
                };
                observe_path_with_metadata(
                    &file.path,
                    &file.metadata,
                    existing_map.get(&path_string),
                    &context,
                )
                .map(|result| result.map(|projection| projection.tracked))
            })
            .collect();

        observations.extend(path_results?.into_iter().flatten());
    }

    let paths_to_remove =
        paths_to_remove_for(&existing_map, &scope, &observations, &excluded_paths);
    reconcile_observations(
        db,
        &existing_map,
        observations,
        &paths_to_remove,
        progress_cb,
    )
}

fn plan_reconciliation(
    existing: &HashMap<String, TrackedFile>,
    observations: Vec<TrackedFile>,
    paths_to_remove: &HashSet<String>,
) -> ReconciliationPlan {
    let mut observed_paths = HashSet::new();
    let mut report = ReconciliationReport::default();
    let mut to_insert = Vec::new();
    let mut to_update = Vec::new();

    for tracked in observations {
        observed_paths.insert(tracked.path.clone());
        match existing.get(&tracked.path) {
            Some(file) if tracked.changed_from(file) => {
                report.updated.push(tracked.path.clone());
                to_update.push(tracked);
            }
            None => {
                report.indexed.push(tracked.path.clone());
                to_insert.push(tracked);
            }
            _ => {}
        };
    }

    let mut to_remove = Vec::new();
    for path in existing.keys() {
        if observed_paths.contains(path) {
            continue;
        }
        if paths_to_remove.contains(path) {
            report.removed.push(path.clone());
            to_remove.push(path.clone());
        }
    }

    ReconciliationPlan {
        report,
        to_insert,
        to_update,
        to_remove,
    }
}

/// Incremental reconciliation for watcher events: processes only the given paths
/// instead of scanning the full watch tree. Used by the debounced event loop.
pub fn reconcile_paths(
    db: &Database,
    paths: Vec<PathBuf>,
) -> Result<IncrementalReconciliationOutcome, AppError> {
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
    let mut arrival_candidates = Vec::new();
    let mut excluded_paths = HashSet::new();

    for path in &paths {
        let path_string = path.to_string_lossy().to_string();

        // Find the matching watch target for scope validation.
        let Some(target) = scope.watch_target_for_path(path) else {
            continue;
        };

        let ignore_set = build_glob_set(&target.ignore_patterns)?;
        let root = PathBuf::from(&target.path);
        let canonical_root = root.canonicalize().ok();

        if target_ignores_path(path, ignore_set.as_ref(), &root, canonical_root.as_deref()) {
            excluded_paths.insert(path_string);
            continue;
        }

        let context = ObservationContext {
            target,
            config: &config,
            rules: &rule_set,
            now,
        };
        if let Some(projection) = observe_path(path, existing_map.get(&path_string), &context)? {
            if !existing_map.contains_key(&path_string) {
                if let Some(candidate) = arrival_rule_candidate(&projection) {
                    arrival_candidates.push(candidate);
                }
            }
            observations.push(projection.tracked);
        }
    }

    let paths_to_remove =
        paths_to_remove_for(&existing_map, &scope, &observations, &excluded_paths);
    let report = reconcile_observations(db, &existing_map, observations, &paths_to_remove, None)?;
    Ok(IncrementalReconciliationOutcome {
        report,
        arrival_candidates,
    })
}

fn observe_path(
    path: &Path,
    existing: Option<&TrackedFile>,
    context: &ObservationContext<'_>,
) -> Result<Option<TrackedRuleProjection>, AppError> {
    // Single stat — symlink_metadata covers both symlink detection and file metadata.
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(None),
    };
    observe_path_with_metadata(path, &metadata, existing, context)
}

fn observe_path_with_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    existing: Option<&TrackedFile>,
    context: &ObservationContext<'_>,
) -> Result<Option<TrackedRuleProjection>, AppError> {
    if is_transient_path(path) {
        return Ok(None);
    }
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(None);
    }
    if is_hidden_path(path, metadata) {
        return Ok(None);
    }
    let tracked =
        tracked_file_from_metadata(path, metadata, existing, context.config, &context.target.id);
    Ok(Some(project_watched_file(
        tracked,
        context.config,
        context.rules,
        context.now,
    )?))
}

fn paths_to_remove_for(
    existing: &HashMap<String, TrackedFile>,
    scope: &PathScope<'_>,
    observations: &[TrackedFile],
    excluded_paths: &HashSet<String>,
) -> HashSet<String> {
    let observed_paths: HashSet<&str> =
        observations.iter().map(|file| file.path.as_str()).collect();

    existing
        .iter()
        .filter(|(path_string, _)| !observed_paths.contains(path_string.as_str()))
        .filter_map(|(path_string, file)| {
            let path = Path::new(path_string);
            (excluded_paths.contains(path_string)
                || !scope.is_tracked_path_active(path, &file.watch_target_id)
                || !path.exists()
                || fs::symlink_metadata(path)
                    .is_ok_and(|metadata| metadata.is_file() && is_hidden_path(path, &metadata)))
            .then(|| path_string.clone())
        })
        .collect()
}

fn reconcile_observations(
    db: &Database,
    existing: &HashMap<String, TrackedFile>,
    observations: Vec<TrackedFile>,
    paths_to_remove: &HashSet<String>,
    progress_cb: Option<&(dyn Fn(usize, usize) + Send + Sync)>,
) -> Result<ReconciliationReport, AppError> {
    let plan = plan_reconciliation(existing, observations, paths_to_remove);
    let ReconciliationPlan {
        report,
        to_insert,
        to_update,
        to_remove,
    } = plan;
    let changes = storage::tracked::TrackedFileChanges {
        inserts: to_insert,
        updates: to_update,
        removes: to_remove,
    };

    if let Some(cb) = progress_cb {
        let total_changes = changes.inserts.len() + changes.updates.len() + changes.removes.len();
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
        storage::tracked::apply_tracked_file_changes_with_progress(
            db,
            changes,
            Some(&progress_emitter),
        )?;
    } else {
        storage::tracked::apply_tracked_file_changes(db, changes)?;
    }

    Ok(report)
}

fn scan_target_paths(
    root: &Path,
    recursive: bool,
    ignore_set: Option<&GlobSet>,
) -> Result<Vec<ScannedFile>, AppError> {
    scan_target_paths_inner(root, recursive, true, ignore_set)
}

fn scan_target_paths_inner(
    root: &Path,
    recursive: bool,
    is_root: bool,
    ignore_set: Option<&GlobSet>,
) -> Result<Vec<ScannedFile>, AppError> {
    let mut files = Vec::new();
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => {
            if is_root {
                return Err(error.into());
            } else {
                return Ok(files);
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
        // On Windows, DirEntry metadata is populated by directory enumeration and
        // does not issue the extra per-path stat performed by symlink_metadata.
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                if is_root {
                    return Err(error.into());
                } else {
                    continue;
                }
            }
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if is_hidden_path(&path, &metadata) {
            continue;
        }
        if metadata.is_dir() && recursive {
            // System directories are always skipped — no override.
            if is_system_directory(&path) {
                continue;
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
            if let Ok(sub_files) = scan_target_paths_inner(&path, recursive, false, ignore_set) {
                files.extend(sub_files);
            }
        } else if metadata.is_file() {
            files.push(ScannedFile { path, metadata });
        }
    }
    Ok(files)
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
    root: &Path,
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
    let relative = path
        .strip_prefix(root)
        .ok()
        .or_else(|| canonical_root.and_then(|root| path.strip_prefix(root).ok()))
        .unwrap_or(path);
    ignore_set.is_match(file_name)
        || ignore_set.is_match(relative)
        || relative.ancestors().skip(1).any(|ancestor| {
            ignore_set.is_match(ancestor)
                || ancestor
                    .file_name()
                    .is_some_and(|name| ignore_set.is_match(name))
        })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use uuid::Uuid;

    use crate::models::{AppConfig, AppError, Expiry, FileDecayState, TrackedFile, WatchTarget};
    use crate::storage::{self, Database};

    use super::{plan_reconciliation, reconcile_paths, reconcile_with_report_with_progress};

    fn tracked(path: &str) -> TrackedFile {
        TrackedFile {
            path: path.into(),
            file_name: path.into(),
            watch_target_id: "watch".into(),
            size_bytes: 1,
            last_observed_mtime: Some(1),
            freshness_at: 1,
            expiry: Expiry::At(2),
            state: FileDecayState::Fresh,
            matched_rule_ids: Vec::new(),
            origin_url: None,
        }
    }

    #[test]
    fn planner_classifies_indexed_updated_and_removed_files_without_storage() {
        let unchanged = tracked("unchanged");
        let mut updated = tracked("updated");
        updated.size_bytes = 2;
        let removed = tracked("removed");
        let existing = [
            (unchanged.path.clone(), unchanged.clone()),
            (updated.path.clone(), tracked("updated")),
            (removed.path.clone(), removed),
        ]
        .into_iter()
        .collect();
        let paths_to_remove = [String::from("removed")].into_iter().collect();

        let plan = plan_reconciliation(
            &existing,
            vec![unchanged, updated, tracked("indexed")],
            &paths_to_remove,
        );

        assert_eq!(plan.report.indexed, vec!["indexed"]);
        assert_eq!(plan.report.updated, vec!["updated"]);
        assert_eq!(plan.report.removed, vec!["removed"]);
        assert_eq!(
            plan.to_insert
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["indexed"]
        );
        assert_eq!(
            plan.to_update
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["updated"]
        );
        assert_eq!(plan.to_remove, vec!["removed"]);
    }

    fn reconcile(db: &Database) -> Result<Vec<String>, AppError> {
        Ok(reconcile_with_report_with_progress(db, None)?.indexed)
    }

    #[cfg(windows)]
    fn set_hidden(path: &Path) {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, INVALID_FILE_ATTRIBUTES,
        };

        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let attributes = unsafe { GetFileAttributesW(path.as_ptr()) };
        assert_ne!(attributes, INVALID_FILE_ATTRIBUTES);
        assert_ne!(
            unsafe { SetFileAttributesW(path.as_ptr(), attributes | FILE_ATTRIBUTE_HIDDEN) },
            0
        );
    }

    #[test]
    fn reconcile_indexes_existing_files_and_removes_deleted_rows() {
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
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
                .expect("tracked lookup should work")
                .is_none()
        );
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
            default_move_destination: None,
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

    #[cfg(windows)]
    #[test]
    fn reconcile_skips_hidden_files() {
        let fixture = Fixture::new();
        let hidden = fixture.write_watch_file("hidden.txt", "hidden");
        set_hidden(&hidden);
        fixture.save_config();

        let indexed = reconcile(&fixture.db).expect("reconciliation should succeed");

        assert!(indexed.is_empty());
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&hidden))
                .expect("tracked lookup should work")
                .is_none()
        );
    }

    #[cfg(windows)]
    #[test]
    fn incremental_reconciliation_removes_file_that_becomes_hidden() {
        let fixture = Fixture::new();
        let file = fixture.write_watch_file("visible.txt", "body");
        fixture.save_config();
        reconcile(&fixture.db).expect("initial reconciliation should succeed");
        set_hidden(&file);

        let report = reconcile_paths(&fixture.db, vec![file.clone()])
            .expect("incremental reconciliation should succeed");

        assert_eq!(report.report.removed, vec![path_string(&file)]);
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
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
    fn reconcile_removes_existing_file_when_ignore_pattern_is_added() {
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
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
                .expect("tracked lookup should work")
                .is_none()
        );
        assert_eq!(report.removed, vec![path_string(&file)]);
    }

    #[test]
    fn incremental_reconciliation_removes_existing_ignored_file() {
        let fixture = Fixture::new();
        let file = fixture.write_watch_file("skip.me", "ignored later");
        fixture.save_config();
        reconcile(&fixture.db).expect("initial reconciliation should succeed");

        fixture.save_config_with_ignore_patterns(vec![String::from("*.me")]);
        let report = reconcile_paths(&fixture.db, vec![file.clone()])
            .expect("incremental reconciliation should succeed");
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
                .expect("tracked lookup should work")
                .is_none()
        );
        assert_eq!(report.report.removed, vec![path_string(&file)]);
    }

    #[test]
    fn reconcile_removes_files_beneath_newly_ignored_directory() {
        let fixture = Fixture::new();
        let ignored_directory = fixture.watch.join("ignored");
        fs::create_dir_all(&ignored_directory).expect("ignored directory should exist");
        let file = ignored_directory.join("keep.txt");
        fs::write(&file, "ignored later").expect("ignored file should exist");
        fixture.save_config_recursive();
        reconcile(&fixture.db).expect("initial reconciliation should succeed");
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
                .expect("tracked lookup should work")
                .is_some()
        );

        fixture.save_config_recursive_with_ignore_patterns(vec![String::from("ignored")]);
        let report = reconcile_with_report_with_progress(&fixture.db, None)
            .expect("reconciliation should succeed");
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
                .expect("tracked lookup should work")
                .is_none()
        );
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
            self.save_config_recursive_with_ignore_patterns(Vec::new());
        }

        fn save_config_recursive_with_ignore_patterns(&self, ignore_patterns: Vec<String>) {
            let config = AppConfig {
                watch_targets: vec![WatchTarget {
                    id: String::from("watch"),
                    path: path_string(&self.watch),
                    enabled: true,
                    recursive: true,
                    ignore_patterns,
                }],
                default_move_destination: None,
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
                }],
                default_move_destination: None,
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
            timing: crate::models::RuleTiming::AfterSeconds(24 * 60 * 60), // 1 day
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
            timing: crate::models::RuleTiming::AfterSeconds(1),
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

        assert_eq!(tracked.state, FileDecayState::RuleIgnored);
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
