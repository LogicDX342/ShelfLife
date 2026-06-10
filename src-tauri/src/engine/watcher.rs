use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use redb::Database;

use crate::models::{AppError, ReconciliationReport, WatchTarget};
use crate::{engine, storage};

pub type ShelflifeDebouncer = Debouncer<notify::RecommendedWatcher, FileIdMap>;
pub type WatcherEventSink = Arc<dyn Fn(WatcherEvent) + Send + Sync + 'static>;

#[derive(Debug, Clone)]
pub enum WatcherEvent {
    Reconciled(ReconciliationReport),
    Error(AppError),
}

pub fn start_watcher(
    db: Arc<Database>,
    targets: &[WatchTarget],
    event_sink: WatcherEventSink,
) -> Result<ShelflifeDebouncer, AppError> {
    let (tx, rx) = mpsc::channel::<DebounceEventResult>();
    let mut debouncer = new_debouncer(Duration::from_millis(500), None, tx).map_err(|error| {
        AppError::with_details(
            "WATCHER_ERROR",
            "File watcher could not be started.",
            true,
            error.to_string(),
        )
    })?;

    for target in targets.iter().filter(|target| target.enabled) {
        let mode = if target.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        debouncer
            .watch(std::path::Path::new(&target.path), mode)
            .map_err(|error| {
                AppError::with_details(
                    "WATCHER_ERROR",
                    "Watch target could not be registered.",
                    true,
                    error.to_string(),
                )
            })?;
    }

    thread::spawn(move || watch_event_loop(db, rx, event_sink));
    Ok(debouncer)
}

pub fn restart_watcher(
    state: &storage::AppState,
    event_sink: WatcherEventSink,
) -> Result<(), AppError> {
    let config = storage::get_config(&state.db)?;
    let mut watcher = state
        .watcher
        .lock()
        .map_err(|_| AppError::new("WATCHER_ERROR", "Watcher state could not be locked.", true))?;

    *watcher = None;
    if state.is_watching_paused() {
        return Ok(());
    }

    *watcher = Some(start_watcher(
        state.db.clone(),
        &config.watch_targets,
        event_sink,
    )?);
    Ok(())
}

pub fn pause_watching(state: &storage::AppState) -> Result<(), AppError> {
    state.set_watching_paused(true);
    let mut watcher = state
        .watcher
        .lock()
        .map_err(|_| AppError::new("WATCHER_ERROR", "Watcher state could not be locked.", true))?;
    *watcher = None;
    Ok(())
}

pub fn resume_watching(
    state: &storage::AppState,
    event_sink: WatcherEventSink,
) -> Result<(), AppError> {
    state.set_watching_paused(false);
    restart_watcher(state, event_sink)
}

fn watch_event_loop(
    db: Arc<Database>,
    rx: mpsc::Receiver<DebounceEventResult>,
    event_sink: WatcherEventSink,
) {
    while let Ok(result) = rx.recv() {
        match result {
            Ok(events) => {
                let paths = events
                    .iter()
                    .flat_map(|event| event.paths.iter().cloned())
                    .collect::<Vec<_>>();

                match process_debounced_paths(&db, paths, Duration::from_secs(1)) {
                    Ok(report) => event_sink(WatcherEvent::Reconciled(report)),
                    Err(error) => event_sink(WatcherEvent::Error(error)),
                }
            }
            Err(errors) => {
                for error in errors {
                    let app_error = AppError::with_details(
                        "WATCHER_ERROR",
                        "File watcher reported an event error.",
                        true,
                        error.to_string(),
                    );
                    event_sink(WatcherEvent::Error(app_error));
                }
            }
        }
    }
}

pub fn process_debounced_paths(
    db: &Database,
    paths: Vec<PathBuf>,
    stability_delay: Duration,
) -> Result<ReconciliationReport, AppError> {
    // Wait for each changed path to stabilise before processing. Already-deleted
    // paths pass through immediately (deletion is a valid stable event).
    let stable_paths = wait_for_paths_stability(paths, stability_delay);
    engine::reconcile_paths(db, stable_paths)
}

/// For each path, poll until size and mtime are stable across two checks or the
/// file has been deleted. Returns only the paths that are ready to process.
fn wait_for_paths_stability(paths: Vec<PathBuf>, delay: Duration) -> Vec<PathBuf> {
    paths
        .into_iter()
        .filter(|path| {
            // Deleted files are stable — they represent a removal event.
            if !path.exists() {
                return true;
            }
            crate::engine::quiescence::wait_for_stability_sync(path, delay, 3)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use uuid::Uuid;

    use crate::models::{AppConfig, FileDecayState, WatchTarget};
    use crate::storage;

    use super::process_debounced_paths;

    #[test]
    fn burst_duplicate_events_index_file_once_in_storage() {
        let fixture = Fixture::new();
        let file = fixture.write_watch_file("burst.txt", "body");
        fixture.save_config(false);

        let report = process_debounced_paths(
            &fixture.db,
            vec![file.clone(), file.clone(), file.clone()],
            Duration::ZERO,
        )
        .expect("burst processing should reconcile");

        assert_eq!(report.indexed, vec![path_string(&file)]);
        assert_eq!(
            storage::tracked::list_tracked_files(&fixture.db)
                .expect("tracked list should work")
                .len(),
            1
        );
    }

    #[test]
    fn rename_style_event_marks_old_path_missing_and_indexes_new_path() {
        let fixture = Fixture::new();
        let old_path = fixture.write_watch_file("old-name.txt", "body");
        fixture.save_config(false);
        process_debounced_paths(&fixture.db, vec![old_path.clone()], Duration::ZERO)
            .expect("initial processing should reconcile");

        let new_path = fixture.watch.join("new-name.txt");
        fs::rename(&old_path, &new_path).expect("test rename should succeed");
        process_debounced_paths(
            &fixture.db,
            vec![old_path.clone(), new_path.clone()],
            Duration::ZERO,
        )
        .expect("rename processing should reconcile");

        let old_tracked = storage::tracked::get_tracked_file(&fixture.db, &path_string(&old_path))
            .expect("old tracked lookup should work")
            .expect("old tracked row should remain as missing");
        let new_tracked = storage::tracked::get_tracked_file(&fixture.db, &path_string(&new_path))
            .expect("new tracked lookup should work")
            .expect("new tracked row should exist");

        assert_eq!(old_tracked.state, FileDecayState::Missing);
        assert_eq!(new_tracked.file_name, "new-name.txt");
    }

    #[test]
    fn recursive_watch_target_indexes_nested_file() {
        let fixture = Fixture::new();
        let nested_dir = fixture.watch.join("nested");
        fs::create_dir_all(&nested_dir).expect("nested directory should be created");
        let nested = nested_dir.join("nested.txt");
        fs::write(&nested, "body").expect("nested file should be written");
        fixture.save_config(true);

        let report = process_debounced_paths(&fixture.db, vec![nested.clone()], Duration::ZERO)
            .expect("recursive processing should reconcile");

        assert_eq!(report.indexed, vec![path_string(&nested)]);
    }

    struct Fixture {
        root: PathBuf,
        watch: PathBuf,
        db: std::sync::Arc<redb::Database>,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("shelflife-watcher-{}", Uuid::new_v4()));
            let watch = root.join("watch");
            fs::create_dir_all(&watch).expect("watch directory should be created");
            let db = storage::open_database(root.join("test.redb")).expect("database should open");
            Self { root, watch, db }
        }

        fn save_config(&self, recursive: bool) {
            let config = AppConfig {
                watch_targets: vec![WatchTarget {
                    id: String::from("watch"),
                    path: path_string(&self.watch),
                    enabled: true,
                    recursive,
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

        fn write_watch_file(&self, name: &str, content: &str) -> PathBuf {
            let path = self.watch.join(name);
            fs::write(&path, content).expect("watch file should be written");
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
