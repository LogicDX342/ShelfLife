use tauri::{AppHandle, Manager, State};

use crate::engine::paths::validate_config_paths;
use crate::models::{AppConfig, AppError, CloseBehavior, WatchTarget};
use crate::runtime::AppRuntime;
use crate::storage;

#[tauri::command]
pub async fn get_config(state: State<'_, AppRuntime>) -> Result<AppConfig, AppError> {
    storage::get_config(&state.db)
}

#[tauri::command]
pub async fn is_reconciliation_active(state: State<'_, AppRuntime>) -> Result<bool, AppError> {
    Ok(state.is_reconciliation_active())
}

#[tauri::command]
pub async fn is_watching_paused(state: State<'_, AppRuntime>) -> Result<bool, AppError> {
    Ok(state.is_watching_paused())
}

#[tauri::command]
pub async fn save_config(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    config: AppConfig,
) -> Result<AppConfig, AppError> {
    validate_config_paths(&config)?;
    storage::save_config(&state.db, &config)?;
    state.sync_after_config_change(&app_handle)?;
    Ok(config)
}

#[tauri::command]
pub async fn resolve_close_request(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    behavior: CloseBehavior,
    remember: bool,
) -> Result<(), AppError> {
    if remember {
        let mut config = storage::get_config(&state.db)?;
        config.close_behavior = behavior.clone();
        storage::save_config(&state.db, &config)?;
    }

    match behavior {
        CloseBehavior::Ask | CloseBehavior::HideToTray => {
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.hide();
            }
        }
        CloseBehavior::Quit => {
            app_handle.exit(0);
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn update_watch_targets(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    targets: Vec<WatchTarget>,
) -> Result<(), AppError> {
    let mut config = storage::get_config(&state.db)?;
    config.watch_targets = targets;
    validate_config_paths(&config)?;
    storage::save_config(&state.db, &config)?;
    state.sync_after_config_change(&app_handle)?;
    Ok(())
}

#[tauri::command]
pub async fn run_reconciliation_scan(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
) -> Result<(), AppError> {
    crate::runtime::reconciliation::run_async_reconciliation(app_handle, state.inner().clone());
    Ok(())
}

#[tauri::command]
pub async fn pause_watching(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
) -> Result<(), AppError> {
    state.pause_watching(&app_handle)
}

#[tauri::command]
pub async fn resume_watching(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
) -> Result<(), AppError> {
    state.resume_watching(&app_handle)
}
