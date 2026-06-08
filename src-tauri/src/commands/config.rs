use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, State};

use crate::engine;
use crate::models::{AppConfig, AppError, ReconciliationReport, WatchTarget};
use crate::storage::{self, AppState};

const PERIODIC_RECONCILIATION_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, AppError> {
    storage::get_config(&state.db)
}

#[tauri::command]
pub async fn save_config(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<AppConfig, AppError> {
    validate_config(&config)?;
    storage::save_config(&state.db, &config)?;
    engine::watcher::restart_watcher(&state, watcher_event_sink(app_handle.clone()))?;
    let report = engine::reconcile_with_report(&state.db)?;
    emit_reconciliation_report(&app_handle, &report);
    Ok(config)
}

#[tauri::command]
pub async fn update_watch_targets(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    targets: Vec<WatchTarget>,
) -> Result<(), AppError> {
    let mut config = storage::get_config(&state.db)?;
    config.watch_targets = targets;
    validate_config(&config)?;
    storage::save_config(&state.db, &config)?;
    engine::watcher::restart_watcher(&state, watcher_event_sink(app_handle.clone()))?;
    let report = engine::reconcile_with_report(&state.db)?;
    emit_reconciliation_report(&app_handle, &report);
    Ok(())
}

#[tauri::command]
pub async fn run_reconciliation_scan(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    let report = engine::reconcile_with_report(&state.db)?;
    emit_reconciliation_report(&app_handle, &report);
    Ok(report.indexed)
}

#[tauri::command]
pub async fn pause_watching(state: State<'_, AppState>) -> Result<(), AppError> {
    state.set_watching_paused(true);
    let mut watcher = state
        .watcher
        .lock()
        .map_err(|_| AppError::new("WATCHER_ERROR", "Watcher state could not be locked.", true))?;
    *watcher = None;
    Ok(())
}

#[tauri::command]
pub async fn resume_watching(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.set_watching_paused(false);
    engine::watcher::restart_watcher(&state, watcher_event_sink(app_handle))
}

pub fn start_periodic_reconciliation(app_handle: AppHandle, state: AppState) {
    thread::spawn(move || loop {
        thread::sleep(PERIODIC_RECONCILIATION_INTERVAL);
        if state.is_watching_paused() {
            continue;
        }

        match engine::reconcile_with_report(&state.db) {
            Ok(report) => emit_reconciliation_report(&app_handle, &report),
            Err(error) => {
                let _ = app_handle.emit("action_failed", error);
            }
        }
    });
}

fn emit_indexed_files(app_handle: &AppHandle, paths: &[String]) {
    for path in paths {
        let _ = app_handle.emit("file_indexed", path);
    }
}

pub fn emit_reconciliation_report(app_handle: &AppHandle, report: &ReconciliationReport) {
    emit_indexed_files(app_handle, &report.indexed);
    for path in &report.updated {
        let _ = app_handle.emit("file_updated", path);
    }
    for path in &report.removed {
        let _ = app_handle.emit("file_removed", path);
    }
    let _ = app_handle.emit("reconciliation_completed", report);
}

pub fn watcher_event_sink(app_handle: AppHandle) -> engine::watcher::WatcherEventSink {
    Arc::new(move |event| match event {
        engine::watcher::WatcherEvent::Reconciled(report) => {
            emit_reconciliation_report(&app_handle, &report);
        }
        engine::watcher::WatcherEvent::Error(error) => {
            let _ = app_handle.emit("action_failed", error);
        }
    })
}

fn validate_config(config: &AppConfig) -> Result<(), AppError> {
    validate_safe_folder(&config.safe_folder_path)?;

    let mut seen_roots = Vec::new();
    for target in config.watch_targets.iter().filter(|target| target.enabled) {
        let canonical = validate_watch_target_path(&target.path)?;
        if seen_roots
            .iter()
            .any(|root: &std::path::PathBuf| root == &canonical)
        {
            return Err(AppError::with_details(
                "PATH_OUT_OF_SCOPE",
                "Duplicate watch target was rejected. No configuration was changed.",
                true,
                target.path.clone(),
            ));
        }
        seen_roots.push(canonical);
    }

    Ok(())
}

fn validate_watch_target_path(path: &str) -> Result<std::path::PathBuf, AppError> {
    let canonical = std::path::PathBuf::from(path)
        .canonicalize()
        .map_err(|error| {
            AppError::with_details(
                "PATH_NOT_FOUND",
                "Watch target must be an existing folder. No configuration was changed.",
                true,
                error.to_string(),
            )
        })?;

    if !canonical.is_dir() {
        return Err(AppError::with_details(
            "PATH_OUT_OF_SCOPE",
            "Watch target must be a folder. No configuration was changed.",
            true,
            canonical.to_string_lossy(),
        ));
    }
    if is_sensitive_root(&canonical) {
        return Err(AppError::with_details(
            "PATH_OUT_OF_SCOPE",
            "Sensitive system or home-root folder was rejected. No configuration was changed.",
            true,
            canonical.to_string_lossy(),
        ));
    }

    Ok(canonical)
}

fn validate_safe_folder(path: &str) -> Result<(), AppError> {
    let path = std::path::PathBuf::from(path);
    if path.as_os_str().is_empty() {
        return Err(AppError::new(
            "PATH_OUT_OF_SCOPE",
            "Safe folder path is required. No configuration was changed.",
            true,
        ));
    }

    let parent = path.parent().ok_or_else(|| {
        AppError::new(
            "PATH_OUT_OF_SCOPE",
            "Safe folder must have a parent folder. No configuration was changed.",
            true,
        )
    })?;

    if parent.exists() && is_sensitive_root(&parent.canonicalize()?) {
        return Err(AppError::with_details(
            "PATH_OUT_OF_SCOPE",
            "Safe folder parent cannot be a sensitive root. No configuration was changed.",
            true,
            parent.to_string_lossy(),
        ));
    }

    Ok(())
}

fn is_sensitive_root(path: &std::path::Path) -> bool {
    if let Ok(home) = std::env::var("USERPROFILE") {
        if path == std::path::Path::new(&home) {
            return true;
        }
    }

    let normalized = path.to_string_lossy().to_lowercase();
    normalized.ends_with("\\windows")
        || normalized.ends_with("\\program files")
        || normalized.ends_with("\\program files (x86)")
        || normalized.ends_with("\\programdata")
        || normalized.ends_with("\\appdata")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use uuid::Uuid;

    use crate::models::{AppConfig, WatchTarget};

    use super::validate_config;

    #[test]
    fn rejects_missing_watch_target() {
        let root = std::env::temp_dir().join(format!("shelflife-config-{}", Uuid::new_v4()));
        let config = AppConfig {
            watch_targets: vec![WatchTarget {
                id: String::from("missing"),
                path: root.join("missing").to_string_lossy().to_string(),
                enabled: true,
                recursive: false,
                default_ttl_seconds: None,
                ignore_patterns: Vec::new(),
                include_hidden_patterns: Vec::new(),
                rule_ids: Vec::new(),
            }],
            safe_folder_path: root.join("safe").to_string_lossy().to_string(),
            ..AppConfig::default()
        };

        let error = validate_config(&config).expect_err("missing path should be rejected");
        assert_eq!(error.code, "PATH_NOT_FOUND");
    }

    #[test]
    fn accepts_existing_watch_target_and_safe_folder_parent() {
        let root = std::env::temp_dir().join(format!("shelflife-config-{}", Uuid::new_v4()));
        let watch = root.join("watch");
        fs::create_dir_all(&watch).expect("watch dir should exist");
        let config = AppConfig {
            watch_targets: vec![WatchTarget {
                id: String::from("watch"),
                path: watch.to_string_lossy().to_string(),
                enabled: true,
                recursive: false,
                default_ttl_seconds: None,
                ignore_patterns: Vec::new(),
                include_hidden_patterns: Vec::new(),
                rule_ids: Vec::new(),
            }],
            safe_folder_path: root.join("safe").to_string_lossy().to_string(),
            ..AppConfig::default()
        };

        validate_config(&config).expect("config should validate");
        let _ = fs::remove_dir_all(root);
    }
}
