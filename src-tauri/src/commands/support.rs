use std::path::Path;

use tauri::{AppHandle, Manager};

use crate::models::AppError;
use crate::runtime::diagnostics;

#[tauri::command]
pub async fn open_diagnostic_logs(app_handle: AppHandle) -> Result<(), AppError> {
    let log_directory = app_handle.path().app_log_dir().map_err(|error| {
        AppError::with_details(
            "APP_LOG_PATH_ERROR",
            "The diagnostic log directory could not be accessed.",
            true,
            error.to_string(),
        )
    })?;

    std::fs::create_dir_all(&log_directory).map_err(|error| {
        AppError::with_details(
            "APP_LOG_PATH_ERROR",
            "The diagnostic log directory could not be created.",
            true,
            error.to_string(),
        )
    })?;

    open_log_directory(&log_directory)?;
    diagnostics::record_event("support", "diagnostic log folder opened");
    Ok(())
}

#[cfg(target_os = "windows")]
fn open_log_directory(path: &Path) -> Result<(), AppError> {
    std::process::Command::new("explorer")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            AppError::with_details(
                "APP_LOG_OPEN_FAILED",
                "The diagnostic log directory could not be opened.",
                true,
                error.to_string(),
            )
        })
}

#[cfg(not(target_os = "windows"))]
fn open_log_directory(_path: &Path) -> Result<(), AppError> {
    Err(AppError::new(
        "APP_LOG_OPEN_FAILED",
        "Opening diagnostic logs is only implemented for Windows v1.",
        true,
    ))
}
