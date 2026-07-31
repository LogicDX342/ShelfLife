use std::path::Path;

use tauri::{AppHandle, Emitter, State};

use crate::engine;
use crate::engine::paths::PathScope;
use crate::models::{
    AppError, AuditEntry, DropzoneActionFailure, DropzoneActionResult, DropzonePreview, RuleAction,
    RuleMode,
};
use crate::rules::{CompiledRuleSet, RuleDecisionScope, RuleVerdict};
use crate::runtime::AppRuntime;
use crate::storage;

#[tauri::command]
pub async fn preview_dropzone_files(
    state: State<'_, AppRuntime>,
    paths: Vec<String>,
) -> Result<DropzonePreview, AppError> {
    crate::dropzone::record_dropzone_drop();
    state.with_database(|db| engine::preview_dropzone_files(db, &paths))
}

#[tauri::command]
pub async fn execute_dropzone_ingest(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    paths: Vec<String>,
    watch_target_id: String,
) -> Result<DropzoneActionResult, AppError> {
    let (result, failure_audits) = state.run_exclusive_engine_operation(|db| {
        let mut result = DropzoneActionResult {
            entries: Vec::new(),
            failures: Vec::new(),
        };
        let mut failure_audits = Vec::new();

        for path in paths {
            match engine::executor::ingest_dropzone_file_audited(db, &path, &watch_target_id) {
                Ok(entry) => result.entries.push(entry),
                Err(failure) => {
                    if let Some(entry) = failure.audit_entry {
                        failure_audits.push(*entry);
                    }
                    result.failures.push(DropzoneActionFailure {
                        path,
                        error: failure.error,
                    });
                }
            }
        }

        Ok::<_, AppError>((result, failure_audits))
    })?;

    emit_dropzone_outcome(&app_handle, &result, &failure_audits, true);
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
    let (result, failure_audits) = state.run_exclusive_engine_operation(|db| {
        let selected_rule = storage::rules::get_rule(db, &rule_id)?.ok_or_else(|| {
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

        let config = storage::get_config(db)?;
        let rule_set = CompiledRuleSet::compile(storage::rules::list_rules(db)?, &config)?;
        let scope = PathScope::new(&config);
        let mut result = DropzoneActionResult {
            entries: Vec::new(),
            failures: Vec::new(),
        };
        let mut failure_audits = Vec::new();

        for path in paths {
            let execution: Result<_, engine::executor::FileActionFailure> = (|| {
                let (_, tracked) = engine::dropzone::build_dropzone_file(&path, &config)?;
                let decision = rule_set.decide_file(
                    &tracked,
                    RuleDecisionScope::Dropzone,
                );
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
                    )
                    .into());
                };
                if effective_rule.id != rule_id {
                    return Err(AppError::new(
                        "RULE_NOT_MATCHED",
                        "The selected rule is no longer the effective match for this file. No file was changed.",
                        true,
                    )
                    .into());
                }
                if matches!(effective_rule.mode, RuleMode::PreviewOnly) {
                    return Err(AppError::new(
                        "RULE_NOT_EXECUTABLE",
                        "PreviewOnly rules cannot change files from the dropzone.",
                        true,
                    )
                    .into());
                }
                if matches!(effective_rule.action, RuleAction::Ignore)
                    && !scope.is_in_enabled_watch_target(Path::new(&path))
                {
                    return Err(AppError::new(
                        "RULE_NOT_EXECUTABLE",
                        "Ignore rules can only change files inside watch targets from the dropzone.",
                        true,
                    )
                    .into());
                }

                let mut explanation = *effective_explanation;
                explanation.message = format!("Dropzone: {}", explanation.message);
                engine::executor::execute_dropzone_rule_action_audited(
                    db,
                    &path,
                    &effective_rule,
                    explanation,
                )
            })();

            match execution {
                Ok(entry) => result.entries.push(entry),
                Err(failure) => {
                    if let Some(entry) = failure.audit_entry {
                        failure_audits.push(*entry);
                    }
                    result.failures.push(DropzoneActionFailure {
                        path,
                        error: failure.error,
                    });
                }
            }
        }

        Ok::<_, AppError>((result, failure_audits))
    })?;

    emit_dropzone_outcome(&app_handle, &result, &failure_audits, false);
    if !result.entries.is_empty() {
        state.wake_rule_scheduler();
    }

    Ok(result)
}

#[tauri::command]
pub async fn hide_dropzone(app_handle: AppHandle) -> Result<(), AppError> {
    crate::dropzone::record_dropzone_drop();
    crate::dropzone::destroy_dropzone(&app_handle)
}

fn emit_dropzone_failures(app_handle: &AppHandle, failures: &[DropzoneActionFailure]) {
    for failure in failures {
        let _ = app_handle.emit("action_failed", &failure.error);
    }
}

fn emit_dropzone_outcome(
    app_handle: &AppHandle,
    result: &DropzoneActionResult,
    failure_audits: &[AuditEntry],
    emit_indexed_destinations: bool,
) {
    for entry in &result.entries {
        let _ = app_handle.emit("action_completed", entry);
        let _ = app_handle.emit("audit_updated", entry);
        if emit_indexed_destinations {
            if let Some(destination) = &entry.destination_path {
                let _ = app_handle.emit("file_indexed", destination);
            }
        }
    }
    for entry in failure_audits {
        let _ = app_handle.emit("audit_updated", entry);
    }
    emit_dropzone_failures(app_handle, &result.failures);
}
