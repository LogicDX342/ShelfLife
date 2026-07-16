use tauri::{AppHandle, Emitter, State};
use tauri_plugin_notification::NotificationExt;

use crate::engine;
use crate::models::{
    AppError, AuditActionKind, AuditEntry, BulkTriageFailure, BulkTriageResult, UndoStatus,
    UserTriageAction,
};
use crate::runtime::AppRuntime;
use crate::storage::{self, Database};

#[tauri::command]
pub async fn execute_triage_action(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    path: String,
    action: UserTriageAction,
) -> Result<AuditEntry, AppError> {
    match state.run_exclusive_engine_operation(|db| {
        engine::executor::execute_triage_action_audited(db, &path, action)
    }) {
        Ok(entry) => {
            let _ = app_handle.emit("action_completed", &entry);
            let _ = app_handle.emit("audit_updated", &entry);
            notify_if_enabled(
                &app_handle,
                &state,
                "Action completed",
                format!(
                    "{} recorded for {}.",
                    audit_action_kind_label(&entry.action_kind),
                    entry.file_name
                ),
            );
            Ok(entry)
        }
        Err(failure) => {
            let error = failure.error;
            if let Some(entry) = failure.audit_entry {
                let _ = app_handle.emit("audit_updated", entry.as_ref());
            }
            let _ = app_handle.emit("action_failed", &error);
            let mut body = format!(
                "{} Review the audit log for the recorded action state.",
                error.message
            );
            if let Some(details) = &error.details {
                body = format!("{} Details: {}", body, details);
            }
            notify_if_enabled(&app_handle, &state, "Action failed", body);
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn confirm_rule_action(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    path: String,
    rule_id: String,
) -> Result<AuditEntry, AppError> {
    match state.run_exclusive_engine_operation(|db| {
        engine::executor::execute_confirmed_rule_action(db, &path, &rule_id)
    }) {
        Ok(entry) => {
            let _ = app_handle.emit("action_completed", &entry);
            let _ = app_handle.emit("audit_updated", &entry);
            notify_if_enabled(
                &app_handle,
                &state,
                "Action completed",
                format!(
                    "{} recorded for {}.",
                    audit_action_kind_label(&entry.action_kind),
                    entry.file_name
                ),
            );
            Ok(entry)
        }
        Err(failure) => {
            let error = failure.error;
            if let Some(entry) = failure.audit_entry {
                let _ = app_handle.emit("audit_updated", entry.as_ref());
            }
            let _ = app_handle.emit("action_failed", &error);
            notify_if_enabled(&app_handle, &state, "Action failed", error.message.clone());
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn execute_bulk_triage_action(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    paths: Vec<String>,
    action: UserTriageAction,
) -> Result<BulkTriageResult, AppError> {
    let (result, failure_audits) = state
        .run_exclusive_engine_operation(|db| execute_bulk_triage_audited(db, paths, action))?;

    for entry in &result.entries {
        let _ = app_handle.emit("action_completed", entry);
        let _ = app_handle.emit("audit_updated", entry);
    }
    for failure in &result.failures {
        let _ = app_handle.emit("action_failed", &failure.error);
    }
    for entry in &failure_audits {
        let _ = app_handle.emit("audit_updated", entry);
    }

    notify_if_enabled(
        &app_handle,
        &state,
        "Bulk action completed",
        format!(
            "{} actions recorded. {} failed.",
            result.entries.len(),
            result.failures.len()
        ),
    );

    Ok(result)
}

fn execute_bulk_triage_audited(
    db: &Database,
    paths: Vec<String>,
    action: UserTriageAction,
) -> Result<(BulkTriageResult, Vec<AuditEntry>), AppError> {
    let mut entries = Vec::new();
    let mut failures = Vec::new();
    let mut failure_audits = Vec::new();

    for path in paths {
        match engine::executor::execute_triage_action_audited(db, &path, action.clone()) {
            Ok(entry) => entries.push(entry),
            Err(failure) => {
                if let Some(entry) = failure.audit_entry {
                    failure_audits.push(*entry);
                }
                failures.push(BulkTriageFailure {
                    path,
                    error: failure.error,
                });
            }
        }
    }

    Ok((BulkTriageResult { entries, failures }, failure_audits))
}

#[tauri::command]
pub async fn undo_audit_entry(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    audit_id: String,
) -> Result<AuditEntry, AppError> {
    match state.run_exclusive_engine_operation(|db| engine::undo_audit_entry(db, &audit_id)) {
        Ok(entry) => {
            let _ = app_handle.emit("audit_updated", &entry);
            notify_if_enabled(
                &app_handle,
                &state,
                "Audit updated",
                format!(
                    "Undo status is now {}.",
                    undo_status_label(&entry.undo_status)
                ),
            );

            // Run reconciliation asynchronously and report progress/completion.
            crate::runtime::reconciliation::run_async_reconciliation(
                app_handle,
                state.inner().clone(),
            );

            Ok(entry)
        }
        Err(error) => {
            let _ = app_handle.emit("action_failed", &error);
            let mut body = error.message.clone();
            if let Some(details) = &error.details {
                body = format!("{} Details: {}", body, details);
            }
            notify_if_enabled(&app_handle, &state, "Undo needs attention", body);
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn list_audit_entries(state: State<'_, AppRuntime>) -> Result<Vec<AuditEntry>, AppError> {
    state.with_database(storage::audit::list_audit_entries)
}

fn notify_if_enabled(
    app_handle: &AppHandle,
    state: &State<'_, AppRuntime>,
    title: &str,
    body: impl Into<String>,
) {
    if state.is_window_visible() {
        return;
    }
    let Ok(config) = state.with_database(storage::get_config) else {
        return;
    };
    if !config.notifications_enabled {
        return;
    }

    let _ = app_handle
        .notification()
        .builder()
        .title(title)
        .body(body.into())
        .show();
}

fn audit_action_kind_label(action_kind: &AuditActionKind) -> &'static str {
    match action_kind {
        AuditActionKind::Trash => "Trash Now",
        AuditActionKind::Move => "Move",
        AuditActionKind::Pin => "Pin",
        AuditActionKind::Snooze => "Snooze",
        AuditActionKind::Ignore => "Ignore",
    }
}

fn undo_status_label(status: &UndoStatus) -> &'static str {
    match status {
        UndoStatus::Available => "available",
        UndoStatus::Unavailable { .. } => "unavailable",
        UndoStatus::Completed => "completed",
        UndoStatus::Failed { .. } => "failed",
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{FileDecayState, UserTriageAction};
    use crate::storage;
    use crate::storage::test_util::{path_string, Fixture};

    use super::execute_bulk_triage_audited;

    #[test]
    fn bulk_triage_records_each_success_and_reports_failures() {
        let fixture = Fixture::new("shelflife-bulk");
        let first = fixture.write_watch_file("first.txt", "first");
        let second = fixture.write_watch_file("second.txt", "second");
        let outside = fixture.write_outside_file("outside.txt", "outside");
        fixture.save_config();

        let (result, _) = execute_bulk_triage_audited(
            &fixture.db,
            vec![
                path_string(&first),
                path_string(&second),
                path_string(&outside),
            ],
            UserTriageAction::Ignore,
        )
        .expect("bulk action should return a result");

        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].error.code, "PATH_OUT_OF_SCOPE");
        assert_eq!(
            storage::audit::list_audit_entries(&fixture.db)
                .expect("audit list should work")
                .len(),
            2
        );
        assert_eq!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&first))
                .expect("tracked lookup should work")
                .expect("tracked file should exist")
                .state,
            FileDecayState::Ignored
        );
        assert!(outside.exists());
    }
}
