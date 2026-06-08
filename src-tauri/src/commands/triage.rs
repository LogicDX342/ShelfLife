use tauri::{AppHandle, Emitter, State};
use tauri_plugin_notification::NotificationExt;

use crate::engine;
use crate::models::{AppError, AuditEntry, BulkTriageFailure, BulkTriageResult, UserTriageAction};
use crate::storage::{self, AppState};

#[tauri::command]
pub async fn execute_triage_action(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    path: String,
    action: UserTriageAction,
) -> Result<AuditEntry, AppError> {
    match engine::execute_triage_action(&state.db, &path, action) {
        Ok(entry) => {
            let _ = app_handle.emit("action_completed", &entry);
            let _ = app_handle.emit("audit_updated", &entry);
            notify_if_enabled(
                &app_handle,
                &state,
                "Action completed",
                format!(
                    "{} recorded for {}.",
                    entry.action_kind.label(),
                    entry.file_name
                ),
            );
            Ok(entry)
        }
        Err(error) => {
            let _ = app_handle.emit("action_failed", &error);
            let mut body = format!("{} No file was silently deleted.", error.message);
            if let Some(details) = &error.details {
                body = format!("{} Details: {}", body, details);
            }
            notify_if_enabled(&app_handle, &state, "Action failed", body);
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn execute_bulk_triage_action(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
    action: UserTriageAction,
) -> Result<BulkTriageResult, AppError> {
    let result = execute_bulk_triage(&state.db, paths, action)?;

    for entry in &result.entries {
        let _ = app_handle.emit("action_completed", entry);
        let _ = app_handle.emit("audit_updated", entry);
    }
    for failure in &result.failures {
        let _ = app_handle.emit("action_failed", &failure.error);
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

fn execute_bulk_triage(
    db: &redb::Database,
    paths: Vec<String>,
    action: UserTriageAction,
) -> Result<BulkTriageResult, AppError> {
    let mut entries = Vec::new();
    let mut failures = Vec::new();

    for path in paths {
        match engine::execute_triage_action(db, &path, action.clone()) {
            Ok(entry) => entries.push(entry),
            Err(error) => failures.push(BulkTriageFailure { path, error }),
        }
    }

    Ok(BulkTriageResult { entries, failures })
}

#[tauri::command]
pub async fn undo_audit_entry(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    audit_id: String,
) -> Result<AuditEntry, AppError> {
    match engine::undo_audit_entry(&state.db, &audit_id) {
        Ok(entry) => {
            let _ = app_handle.emit("audit_updated", &entry);
            notify_if_enabled(
                &app_handle,
                &state,
                "Audit updated",
                format!("Undo status is now {}.", entry.undo_status.label()),
            );

            // Run reconciliation and emit report immediately to update file expiries/states.
            if let Ok(report) = engine::reconcile_with_report(&state.db) {
                crate::commands::config::emit_reconciliation_report(&app_handle, &report);
            }

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
pub async fn list_audit_entries(state: State<'_, AppState>) -> Result<Vec<AuditEntry>, AppError> {
    storage::audit::list_audit_entries(&state.db)
}

fn notify_if_enabled(
    app_handle: &AppHandle,
    state: &State<'_, AppState>,
    title: &str,
    body: impl Into<String>,
) {
    let Ok(config) = storage::get_config(&state.db) else {
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

trait AuditActionKindLabel {
    fn label(&self) -> &'static str;
}

impl AuditActionKindLabel for crate::models::AuditActionKind {
    fn label(&self) -> &'static str {
        match self {
            crate::models::AuditActionKind::Trash => "Trash Now",
            crate::models::AuditActionKind::Move => "Move",
            crate::models::AuditActionKind::Rename => "Rename",
            crate::models::AuditActionKind::Pin => "Pin",
            crate::models::AuditActionKind::Snooze => "Snooze",
            crate::models::AuditActionKind::Ignore => "Ignore",
            crate::models::AuditActionKind::RulePreview => "Rule preview",
        }
    }
}

trait UndoStatusLabel {
    fn label(&self) -> &'static str;
}

impl UndoStatusLabel for crate::models::UndoStatus {
    fn label(&self) -> &'static str {
        match self {
            crate::models::UndoStatus::Available => "available",
            crate::models::UndoStatus::Unavailable { .. } => "unavailable",
            crate::models::UndoStatus::Completed => "completed",
            crate::models::UndoStatus::Failed { .. } => "failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use uuid::Uuid;

    use crate::models::{AppConfig, FileDecayState, UserTriageAction, WatchTarget};
    use crate::storage;

    use super::execute_bulk_triage;

    #[test]
    fn bulk_triage_records_each_success_and_reports_failures() {
        let fixture = Fixture::new();
        let first = fixture.write_watch_file("first.txt", "first");
        let second = fixture.write_watch_file("second.txt", "second");
        let outside = fixture.write_outside_file("outside.txt", "outside");
        fixture.save_config();

        let result = execute_bulk_triage(
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

    struct Fixture {
        root: PathBuf,
        watch: PathBuf,
        outside: PathBuf,
        safe: PathBuf,
        db: std::sync::Arc<redb::Database>,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("shelflife-bulk-{}", Uuid::new_v4()));
            let watch = root.join("watch");
            let outside = root.join("outside");
            let safe = root.join("safe");
            fs::create_dir_all(&watch).expect("watch directory should be created");
            fs::create_dir_all(&outside).expect("outside directory should be created");
            fs::create_dir_all(&safe).expect("safe directory should be created");
            let db = storage::open_database(root.join("test.redb")).expect("database should open");
            Self {
                root,
                watch,
                outside,
                safe,
                db,
            }
        }

        fn save_config(&self) {
            let config = AppConfig {
                watch_targets: vec![WatchTarget {
                    id: String::from("watch"),
                    path: path_string(&self.watch),
                    enabled: true,
                    recursive: false,
                    default_ttl_seconds: None,
                    ignore_patterns: Vec::new(),
                    include_hidden_patterns: Vec::new(),
                    rule_ids: Vec::new(),
                }],
                safe_folder_path: path_string(&self.safe),
                ..AppConfig::default()
            };
            storage::save_config(&self.db, &config).expect("config should save");
        }

        fn write_watch_file(&self, name: &str, content: &str) -> PathBuf {
            self.write_file(&self.watch.join(name), content)
        }

        fn write_outside_file(&self, name: &str, content: &str) -> PathBuf {
            self.write_file(&self.outside.join(name), content)
        }

        fn write_file(&self, path: &Path, content: &str) -> PathBuf {
            fs::write(path, content).expect("test file should be written");
            path.to_path_buf()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn path_string(path: &Path) -> String {
        path.to_string_lossy().to_string()
    }
}
