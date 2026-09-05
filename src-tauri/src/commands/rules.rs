use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::models::{AppError, AutomationRule, RuleMatchExplanation};
use crate::rules::CompiledRuleSet;
use crate::runtime::AppRuntime;
use crate::storage::{self, Database};

#[tauri::command]
pub async fn list_rules(state: State<'_, AppRuntime>) -> Result<Vec<AutomationRule>, AppError> {
    state.with_database(storage::rules::list_rules)
}

#[tauri::command]
pub async fn save_rule(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    mut rule: AutomationRule,
) -> Result<AutomationRule, AppError> {
    let now = crate::engine::now_seconds();
    if rule.id.is_empty() {
        rule.id = Uuid::new_v4().to_string();
        rule.created_at = now;
    }
    rule.updated_at = now;

    state.run_exclusive_engine_operation(|db| {
        let config = storage::get_config(db)?;
        CompiledRuleSet::compile(vec![rule.clone()], &config)?;
        storage::rules::save_rule(db, &rule)?;
        crate::engine::refresh_tracked_rule_state(db)
    })?;
    crate::runtime::reconciliation::emit_reconciliation_completed(&app_handle);
    state.wake_rule_scheduler();
    crate::runtime::rule_scheduler::run_async_expired_rule_execution(
        app_handle,
        state.inner().clone(),
    );

    Ok(rule)
}

#[tauri::command]
pub async fn test_rule(
    _app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    rule: AutomationRule,
) -> Result<Vec<RuleMatchExplanation>, AppError> {
    state.with_database(|db| {
        let matched_explanations = build_rule_preview_explanations(db, &rule)?
            .into_iter()
            .filter(|exp| exp.proposed_action.is_some())
            .collect();
        Ok(matched_explanations)
    })
}

fn build_rule_preview_explanations(
    db: &Database,
    rule: &AutomationRule,
) -> Result<Vec<RuleMatchExplanation>, AppError> {
    let config = storage::get_config(db)?;
    let files = storage::tracked::list_tracked_files(db)?;
    let mut test_rule = rule.clone();
    test_rule.enabled = true;
    let rule_set = CompiledRuleSet::compile(vec![test_rule], &config)?;
    let mut explanations = Vec::new();

    for file in files {
        explanations.extend(rule_set.explain_file(&file));
    }

    Ok(explanations)
}

#[tauri::command]
pub async fn delete_rule(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    id: String,
) -> Result<(), AppError> {
    state.run_exclusive_engine_operation(|db| {
        storage::rules::delete_rule(db, &id)?;
        crate::engine::refresh_tracked_rule_state(db)
    })?;
    crate::runtime::reconciliation::emit_reconciliation_completed(&app_handle);
    state.wake_rule_scheduler();

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::models::{RuleAction, SizeCondition};
    use crate::rules::CompiledRuleSet;
    use crate::storage;
    use crate::storage::test_util::{path_string, Fixture};

    use super::build_rule_preview_explanations;

    #[test]
    fn rule_preview_returns_explanations_without_writing_audit_rows() {
        let fixture = Fixture::new("shelflife-rule");
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.save_config();
        fixture.track_file(&file);

        let rule = fixture.rule();

        let explanations =
            build_rule_preview_explanations(&fixture.db, &rule).expect("preview should build");

        assert!(file.exists());
        assert_eq!(explanations.len(), 1);
        assert_eq!(
            storage::audit::list_audit_entries_page(&fixture.db, None, "")
                .expect("audit list should work")
                .entries
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

        let error = CompiledRuleSet::compile(vec![rule], &config)
            .err()
            .expect("invalid regex should be rejected");

        assert_eq!(error.code, "RULE_INVALID_REGEX");
    }

    #[test]
    fn invalid_rule_glob_uses_glob_error_code() {
        let fixture = Fixture::new("shelflife-rule");
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.conditions.filename_globs = vec![String::from("[")];

        let error = CompiledRuleSet::compile(vec![rule], &config)
            .err()
            .expect("invalid glob should be rejected");

        assert_eq!(error.code, "RULE_INVALID_GLOB");
    }

    #[test]
    fn invalid_rule_size_range_uses_size_error_code() {
        let fixture = Fixture::new("shelflife-rule");
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.conditions.size = SizeCondition::Between { min: 10, max: 1 };

        let error = CompiledRuleSet::compile(vec![rule], &config)
            .err()
            .expect("invalid size range should be rejected");

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

        CompiledRuleSet::compile(vec![rule], &config).expect("outside destination should validate");
    }

    #[test]
    fn rule_watch_path_outside_configured_targets_is_rejected() {
        let fixture = Fixture::new("shelflife-rule");
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.watch_path = path_string(&fixture.root.join("outside"));

        let error = CompiledRuleSet::compile(vec![rule], &config)
            .err()
            .expect("outside watch path should fail");

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

        let error = CompiledRuleSet::compile(vec![rule], &config)
            .err()
            .expect("in-watch destination should fail");

        assert_eq!(error.code, "MOVE_DESTINATION_WATCHED");
    }
}
