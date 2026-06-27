use serde::Serialize;
use tauri::{ipc::Channel, AppHandle};
use tauri_plugin_updater::UpdaterExt;

use crate::models::AppError;

#[derive(Serialize)]
pub struct AppUpdate {
    pub version: String,
    pub current_version: String,
}

#[derive(Serialize)]
#[serde(tag = "event", content = "data")]
pub enum AppUpdateEvent {
    #[serde(rename_all = "camelCase")]
    Progress {
        chunk_length: usize,
        content_length: Option<u64>,
    },
    Finished,
}

#[tauri::command]
pub async fn check_for_update(app_handle: AppHandle) -> Result<Option<AppUpdate>, AppError> {
    let updater = app_handle.updater().map_err(update_error)?;
    let update = updater.check().await.map_err(update_error)?;

    Ok(update.map(|update| AppUpdate {
        version: update.version,
        current_version: update.current_version,
    }))
}

#[tauri::command]
pub async fn install_update(
    app_handle: AppHandle,
    on_event: Channel<AppUpdateEvent>,
) -> Result<(), AppError> {
    let updater = app_handle.updater().map_err(update_error)?;
    let Some(update) = updater.check().await.map_err(update_error)? else {
        return Err(AppError::new(
            "UPDATE_NOT_AVAILABLE",
            "No update is currently available.",
            true,
        ));
    };

    update
        .download_and_install(
            |chunk_length, content_length| {
                let _ = on_event.send(AppUpdateEvent::Progress {
                    chunk_length,
                    content_length,
                });
            },
            || {
                let _ = on_event.send(AppUpdateEvent::Finished);
            },
        )
        .await
        .map_err(update_error)
}

fn update_error(error: tauri_plugin_updater::Error) -> AppError {
    AppError::with_details(
        "UPDATE_ERROR",
        "Update operation failed.",
        true,
        error.to_string(),
    )
}
