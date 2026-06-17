use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::engine::paths::root_contains;
use crate::models::{
    AppConfig, AppError, AuditActionKind, AuditEntry, AutomationRule, RuleAction,
    RuleMatchExplanation, RuleMode, SizeCondition, UndoStatus,
};
use crate::rules::explain_file_against_rules;
use crate::storage::{self, AppState};

#[tauri::command]
pub async fn list_rules(state: State<'_, AppState>) -> Result<Vec<AutomationRule>, AppError> {
    storage::rules::list_rules(&state.db)
}

#[tauri::command]
pub async fn save_rule(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    mut rule: AutomationRule,
) -> Result<AutomationRule, AppError> {
    let config = storage::get_config(&state.db)?;
    let now = crate::engine::now_seconds();
    if rule.id.trim().is_empty() {
        rule.id = Uuid::new_v4().to_string();
        rule.created_at = now;
        rule.mode = RuleMode::PreviewOnly;
    }
    rule.updated_at = now;
    validate_rule(&rule, &config)?;
    storage::rules::save_rule(&state.db, &rule)?;

    let report = crate::engine::refresh_tracked_rule_state(&state.db)?;
    crate::commands::config::emit_reconciliation_report(&app_handle, &report);
    state.wake_rule_scheduler();
    crate::commands::automation::run_async_expired_rule_execution(
        app_handle,
        state.inner().clone(),
    );

    Ok(rule)
}

#[tauri::command]
pub async fn test_rule(
    _app_handle: AppHandle,
    state: State<'_, AppState>,
    rule: AutomationRule,
) -> Result<Vec<RuleMatchExplanation>, AppError> {
    validate_rule(&rule, &storage::get_config(&state.db)?)?;
    let (explanations, _entries) = build_rule_preview_entries(&state.db, &rule)?;
    let matched_explanations = explanations
        .into_iter()
        .filter(|exp| exp.proposed_action.is_some())
        .collect();
    Ok(matched_explanations)
}

fn build_rule_preview_entries(
    db: &redb::Database,
    rule: &AutomationRule,
) -> Result<(Vec<RuleMatchExplanation>, Vec<AuditEntry>), AppError> {
    let config = storage::get_config(db)?;

    let files = storage::tracked::list_tracked_files(db)?;
    let mut explanations = Vec::new();
    let mut entries = Vec::new();
    let mut test_rule = rule.clone();
    test_rule.enabled = true;
    for file in files {
        let file_explanations =
            explain_file_against_rules(&file, &config, std::slice::from_ref(&test_rule))?;
        for explanation in &file_explanations {
            if explanation.proposed_action.is_some() {
                let entry = AuditEntry {
                    id: Uuid::new_v4().to_string(),
                    sequence: 0,
                    timestamp: crate::engine::now_seconds(),
                    action_kind: AuditActionKind::RulePreview,
                    source_path: file.path.clone(),
                    destination_path: None,
                    file_name: file.file_name.clone(),
                    size_bytes: file.size_bytes,
                    rule_id: Some(rule.id.clone()),
                    rule_name: Some(rule.name.clone()),
                    explanation: Some(explanation.clone()),
                    undo_status: UndoStatus::Unavailable {
                        reason: String::from("Preview did not change the file."),
                    },
                };
                entries.push(entry);
            }
        }
        explanations.extend(file_explanations);
    }

    Ok((explanations, entries))
}

#[tauri::command]
pub async fn delete_rule(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AppError> {
    storage::rules::delete_rule(&state.db, &id)?;

    let report = crate::engine::refresh_tracked_rule_state(&state.db)?;
    crate::commands::config::emit_reconciliation_report(&app_handle, &report);
    state.wake_rule_scheduler();

    Ok(())
}

fn validate_rule(rule: &AutomationRule, config: &AppConfig) -> Result<(), AppError> {
    for pattern in &rule.conditions.filename_regexes {
        regex::Regex::new(pattern).map_err(|error| {
            AppError::with_details(
                "RULE_INVALID_REGEX",
                "Filename regex could not be parsed. Rule was not saved.",
                true,
                error.to_string(),
            )
        })?;
    }

    for glob in &rule.conditions.filename_globs {
        globset::Glob::new(glob).map_err(|error| {
            AppError::with_details(
                "RULE_INVALID_GLOB",
                "Filename glob could not be parsed. Rule was not saved.",
                true,
                error.to_string(),
            )
        })?;
    }

    if let SizeCondition::Between { min, max } = &rule.conditions.size {
        if min > max {
            return Err(AppError::new(
                "RULE_INVALID_SIZE_RANGE",
                "Size range minimum cannot exceed maximum. Rule was not saved.",
                true,
            ));
        }
    }

    if !config
        .watch_targets
        .iter()
        .filter(|target| target.enabled)
        .any(|target| root_contains(&target.path, &rule.watch_path))
    {
        return Err(AppError::path_out_of_scope(&rule.watch_path));
    }

    if let RuleAction::Move {
        destination_folder,
        rename_template,
    } = &rule.action
    {
        let destination = std::path::PathBuf::from(destination_folder);
        crate::engine::validate_move_destination_folder(&destination, config)?;
        if let Some(template) = rename_template {
            crate::engine::validate_rename_template(template)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::models::{RuleAction, SizeCondition};
    use crate::storage;
    use crate::storage::test_util::{path_string, Fixture};

    use super::{build_rule_preview_entries, validate_rule};

    #[test]
    fn rule_preview_creates_audit_rows_without_changing_files() {
        let fixture = Fixture::new("shelflife-rule");
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.save_config();
        fixture.track_file(&file);

        let rule = fixture.rule();

        let (explanations, entries) =
            build_rule_preview_entries(&fixture.db, &rule).expect("preview should build");

        assert!(file.exists());
        assert_eq!(entries.len(), 1);
        assert_eq!(explanations.len(), 1);
        assert_eq!(
            storage::audit::list_audit_entries(&fixture.db)
                .expect("audit list should work")
                .len(),
            0
        );
    }

    #[test]
    fn invalid_rule_regex_is_rejected_before_save() {
        let fixture = Fixture::new("shelflife-rule");
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.conditions.filename_regexes = vec![String::from("[")];

        let error = validate_rule(&rule, &config).expect_err("invalid regex should be rejected");

        assert_eq!(error.code, "RULE_INVALID_REGEX");
    }

    #[test]
    fn invalid_rule_glob_uses_glob_error_code() {
        let fixture = Fixture::new("shelflife-rule");
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.conditions.filename_globs = vec![String::from("[")];

        let error = validate_rule(&rule, &config).expect_err("invalid glob should be rejected");

        assert_eq!(error.code, "RULE_INVALID_GLOB");
    }

    #[test]
    fn invalid_rule_size_range_uses_size_error_code() {
        let fixture = Fixture::new("shelflife-rule");
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.conditions.size = SizeCondition::Between { min: 10, max: 1 };

        let error =
            validate_rule(&rule, &config).expect_err("invalid size range should be rejected");

        assert_eq!(error.code, "RULE_INVALID_SIZE_RANGE");
    }

    #[test]
    fn move_rule_destination_can_be_uncreated_folder_outside_watch_targets() {
        let fixture = Fixture::new("shelflife-rule");
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.action = RuleAction::Move {
            destination_folder: path_string(&fixture.outside.join("archive")),
            rename_template: None,
        };

        validate_rule(&rule, &config).expect("outside destination should validate");
    }

    #[test]
    fn rule_watch_path_outside_configured_targets_is_rejected() {
        let fixture = Fixture::new("shelflife-rule");
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.watch_path = path_string(&fixture.root.join("outside"));

        let error = validate_rule(&rule, &config).expect_err("outside watch path should fail");

        assert_eq!(error.code, "PATH_OUT_OF_SCOPE");
    }

    #[test]
    fn move_rule_destination_inside_watch_target_is_rejected() {
        let fixture = Fixture::new("shelflife-rule");
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.action = RuleAction::Move {
            destination_folder: path_string(&fixture.watch.join("archive")),
            rename_template: None,
        };

        let error = validate_rule(&rule, &config).expect_err("in-watch destination should fail");

        assert_eq!(error.code, "RULE_INVALID_DESTINATION");
    }

    #[test]
    fn move_rule_unknown_rename_placeholder_is_rejected() {
        let fixture = Fixture::new("shelflife-rule");
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.action = RuleAction::Move {
            destination_folder: path_string(&fixture.outside),
            rename_template: Some(String::from("{month}-{file}")),
        };

        let error = validate_rule(&rule, &config).expect_err("unknown placeholder should fail");

        assert_eq!(error.code, "RULE_INVALID_RENAME_TEMPLATE");
    }
}
