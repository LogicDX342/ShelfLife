use tauri::{AppHandle, Emitter, Manager, State};

use crate::engine;
use crate::models::{
    AppError, DropzoneActionFailure, DropzoneActionResult, DropzonePreview, RuleMode,
};
use crate::rules::conditions::evaluate_conditions;
use crate::rules::explanation::rule_explanation;
use crate::runtime::AppRuntime;
use crate::storage;

#[tauri::command]
pub async fn preview_dropzone_files(
    state: State<'_, AppRuntime>,
    paths: Vec<String>,
) -> Result<DropzonePreview, AppError> {
    crate::dropzone::record_dropzone_drop();
    engine::preview_dropzone_files(&state.db, &paths)
}

#[tauri::command]
pub async fn execute_dropzone_ingest(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    paths: Vec<String>,
    watch_target_id: String,
) -> Result<DropzoneActionResult, AppError> {
    let mut result = DropzoneActionResult {
        entries: Vec::new(),
        failures: Vec::new(),
    };

    for path in paths {
        match engine::ingest_dropzone_file(&state.db, &path, &watch_target_id) {
            Ok(entry) => {
                let _ = app_handle.emit("action_completed", &entry);
                let _ = app_handle.emit("audit_updated", &entry);
                if let Some(destination) = &entry.destination_path {
                    let _ = app_handle.emit("file_indexed", destination);
                }
                result.entries.push(entry);
            }
            Err(error) => result.failures.push(DropzoneActionFailure { path, error }),
        }
    }

    emit_dropzone_failures(&app_handle, &result.failures);
    if !result.entries.is_empty() {
        state.wake_rule_scheduler();
        crate::runtime::rule_scheduler::run_async_expired_rule_execution(
            app_handle,
            state.inner().clone(),
        );
    }

    Ok(result)
}

#[tauri::command]
pub async fn execute_dropzone_rule_group(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    rule_id: String,
    paths: Vec<String>,
) -> Result<DropzoneActionResult, AppError> {
    let rule = storage::rules::get_rule(&state.db, &rule_id)?.ok_or_else(|| {
        AppError::new(
            "RULE_NOT_FOUND",
            "Selected rule is no longer available. No file was changed.",
            true,
        )
    })?;

    if matches!(rule.mode, RuleMode::PreviewOnly) {
        return Err(AppError::new(
            "RULE_NOT_EXECUTABLE",
            "PreviewOnly rules cannot change files from the dropzone.",
            true,
        ));
    }

    let config = storage::get_config(&state.db)?;
    let mut result = DropzoneActionResult {
        entries: Vec::new(),
        failures: Vec::new(),
    };

    for path in paths {
        let execution = (|| {
            let (_, tracked) = engine::dropzone::build_dropzone_file(&path, &config)?;
            let condition_match = evaluate_conditions(
                &tracked.file_name,
                tracked.size_bytes,
                &tracked.origin,
                &rule.conditions,
            )?;
            if !condition_match.matched {
                return Err(AppError::new(
                    "RULE_NOT_MATCHED",
                    "The selected rule no longer matches this file. No file was changed.",
                    true,
                ));
            }

            let mut explanation =
                rule_explanation(&tracked.path, tracked.size_bytes, &rule, condition_match);
            explanation.message = format!("Dropzone: {}", explanation.message);
            engine::execute_dropzone_rule_action(&state.db, &path, &rule, explanation)
        })();

        match execution {
            Ok(entry) => {
                let _ = app_handle.emit("action_completed", &entry);
                let _ = app_handle.emit("audit_updated", &entry);
                result.entries.push(entry);
            }
            Err(error) => result.failures.push(DropzoneActionFailure { path, error }),
        }
    }

    emit_dropzone_failures(&app_handle, &result.failures);
    if !result.entries.is_empty() {
        state.wake_rule_scheduler();
    }

    Ok(result)
}

#[tauri::command]
pub async fn hide_dropzone(app_handle: AppHandle) -> Result<(), AppError> {
    crate::dropzone::record_dropzone_drop();
    if let Some(window) = app_handle.get_webview_window("dropzone") {
        window.hide().map_err(|error| {
            AppError::with_details(
                "ACTION_FAILED",
                "Dropzone window could not be hidden.",
                true,
                error.to_string(),
            )
        })?;
    }

    Ok(())
}

fn emit_dropzone_failures(app_handle: &AppHandle, failures: &[DropzoneActionFailure]) {
    for failure in failures {
        let _ = app_handle.emit("action_failed", &failure.error);
    }
}
