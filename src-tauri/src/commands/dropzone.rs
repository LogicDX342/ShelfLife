use std::path::Path;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::engine;
use crate::engine::paths::PathScope;
use crate::models::{
    AppError, DropzoneActionFailure, DropzoneActionResult, DropzonePreview, RuleAction, RuleMode,
};
use crate::rules::{decide_file_against_rules, RuleDecisionScope, RuleVerdict};
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
    let selected_rule = storage::rules::get_rule(&state.db, &rule_id)?.ok_or_else(|| {
        AppError::new(
            "RULE_NOT_FOUND",
            "Selected rule is no longer available. No file was changed.",
            true,
        )
    })?;

    if matches!(selected_rule.mode, RuleMode::PreviewOnly) {
        return Err(AppError::new(
            "RULE_NOT_EXECUTABLE",
            "PreviewOnly rules cannot change files from the dropzone.",
            true,
        ));
    }

    let config = storage::get_config(&state.db)?;
    let rules = storage::rules::list_rules(&state.db)?;
    let scope = PathScope::new(&config);
    let mut result = DropzoneActionResult {
        entries: Vec::new(),
        failures: Vec::new(),
    };

    for path in paths {
        let execution = (|| {
            let (_, tracked) = engine::dropzone::build_dropzone_file(&path, &config)?;
            let decision =
                decide_file_against_rules(&tracked, &config, &rules, RuleDecisionScope::Dropzone)?;
            let RuleVerdict::Matched {
                effective_rule,
                effective_explanation,
                ..
            } = decision.verdict
            else {
                return Err(AppError::new(
                        "RULE_NOT_MATCHED",
                        "The selected rule is no longer the effective match for this file. No file was changed.",
                        true,
                    ));
            };
            if effective_rule.id != rule_id {
                return Err(AppError::new(
                        "RULE_NOT_MATCHED",
                        "The selected rule is no longer the effective match for this file. No file was changed.",
                        true,
                    ));
            }
            if matches!(effective_rule.mode, RuleMode::PreviewOnly) {
                return Err(AppError::new(
                    "RULE_NOT_EXECUTABLE",
                    "PreviewOnly rules cannot change files from the dropzone.",
                    true,
                ));
            }
            if matches!(effective_rule.action, RuleAction::Ignore)
                && !scope.is_in_enabled_watch_target(Path::new(&path))
            {
                return Err(AppError::new(
                    "RULE_NOT_EXECUTABLE",
                    "Ignore rules can only change files inside watch targets from the dropzone.",
                    true,
                ));
            }

            let mut explanation = *effective_explanation;
            explanation.message = format!("Dropzone: {}", explanation.message);
            engine::execute_dropzone_rule_action(&state.db, &path, &effective_rule, explanation)
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
