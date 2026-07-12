use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};

use crate::models::{AppError, WatchTarget};

pub type ShelflifeDebouncer = Debouncer<RecommendedWatcher, FileIdMap>;
pub type WatcherEventSink = Arc<dyn Fn(WatcherEvent) + Send + Sync + 'static>;

#[derive(Debug, Clone)]
pub enum WatcherEvent {
    PathsReady(Vec<PathBuf>),
    Error(AppError),
}

pub fn start_watcher(
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

    thread::spawn(move || watch_event_loop(rx, event_sink));
    Ok(debouncer)
}

fn watch_event_loop(rx: mpsc::Receiver<DebounceEventResult>, event_sink: WatcherEventSink) {
    while let Ok(result) = rx.recv() {
        match result {
            Ok(events) => {
                let paths = events
                    .iter()
                    .flat_map(|event| event.paths.iter().cloned())
                    .collect::<Vec<_>>();

                let stable_paths = wait_for_paths_stability(paths, Duration::from_secs(1));
                event_sink(WatcherEvent::PathsReady(stable_paths));
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
    use std::path::PathBuf;
    use std::time::Duration;

    use uuid::Uuid;

    use super::wait_for_paths_stability;

    #[test]
    fn deleted_paths_pass_through_as_stable_events() {
        let fixture = Fixture::new();
        let path = fixture.watch.join("missing.txt");

        let paths = wait_for_paths_stability(vec![path.clone()], Duration::ZERO);

        assert_eq!(paths, vec![path]);
    }

    struct Fixture {
        root: PathBuf,
        watch: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("shelflife-watcher-{}", Uuid::new_v4()));
            let watch = root.join("watch");
            fs::create_dir_all(&watch).expect("watch directory should be created");
            Self { root, watch }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
