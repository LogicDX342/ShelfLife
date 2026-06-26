use tauri::AppHandle;

use crate::models::AppError;
use crate::tray::TrayLabels;

#[tauri::command]
pub async fn update_tray_labels(app_handle: AppHandle, labels: TrayLabels) -> Result<(), AppError> {
    crate::tray::update_tray_labels(&app_handle, labels)
}
