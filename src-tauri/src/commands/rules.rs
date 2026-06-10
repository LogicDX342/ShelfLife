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

    // Run reconciliation and emit report immediately to update file expiries/states.
    if let Ok(report) = crate::engine::reconcile_with_report(&state.db) {
        crate::commands::config::emit_reconciliation_report(&app_handle, &report);
    }

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

    // Run reconciliation and emit report immediately to update file expiries/states.
    if let Ok(report) = crate::engine::reconcile_with_report(&state.db) {
        crate::commands::config::emit_reconciliation_report(&app_handle, &report);
    }

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

    if let RuleAction::Move { destination_path } = &rule.action {
        let destination = std::path::PathBuf::from(destination_path);
        let has_valid_parent = destination.parent().is_some_and(|parent| {
            config
                .watch_targets
                .iter()
                .filter(|target| target.enabled)
                .any(|target| root_contains(&target.path, parent))
                || root_contains(&config.safe_folder_path, parent)
        });

        if !has_valid_parent {
            return Err(AppError::with_details(
                "RULE_INVALID_DESTINATION",
                "Move destination must be inside a watch target or safe folder. Rule was not saved.",
                true,
                destination_path,
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use uuid::Uuid;

    use crate::engine::freshness::tracked_file_from_metadata;
    use crate::models::{
        AppConfig, AutomationRule, OriginEvidence, RuleAction, RuleConditions, RuleMode,
        SizeCondition, WatchTarget,
    };
    use crate::storage;

    use super::{build_rule_preview_entries, validate_rule};

    #[test]
    fn rule_preview_creates_audit_rows_without_changing_files() {
        let fixture = Fixture::new();
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.save_config();
        fixture.track_file(&file);

        let rule = AutomationRule {
            id: String::from("zip-rule"),
            name: String::from("Zip downloads"),
            enabled: true,
            priority: 10,
            watch_path: path_string(&fixture.watch),
            ttl_seconds: 86_400,
            conditions: RuleConditions {
                extensions: vec![String::from("zip")],
                filename_globs: Vec::new(),
                filename_regexes: Vec::new(),
                source_domains: Vec::new(),
                size: SizeCondition::Any,
            },
            action: RuleAction::Trash,
            mode: RuleMode::PreviewOnly,
            created_at: 1,
            updated_at: 1,
        };

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
        let fixture = Fixture::new();
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.conditions.filename_regexes = vec![String::from("[")];

        let error = validate_rule(&rule, &config).expect_err("invalid regex should be rejected");

        assert_eq!(error.code, "RULE_INVALID_REGEX");
    }

    #[test]
    fn invalid_rule_glob_uses_glob_error_code() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.conditions.filename_globs = vec![String::from("[")];

        let error = validate_rule(&rule, &config).expect_err("invalid glob should be rejected");

        assert_eq!(error.code, "RULE_INVALID_GLOB");
    }

    #[test]
    fn invalid_rule_size_range_uses_size_error_code() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.conditions.size = SizeCondition::Between { min: 10, max: 1 };

        let error =
            validate_rule(&rule, &config).expect_err("invalid size range should be rejected");

        assert_eq!(error.code, "RULE_INVALID_SIZE_RANGE");
    }

    #[test]
    fn move_rule_destination_can_be_inside_uncreated_safe_folder() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.action = RuleAction::Move {
            destination_path: path_string(&fixture.root.join("safe").join("download.zip")),
        };

        validate_rule(&rule, &config).expect("safe folder destination should validate");
    }

    #[test]
    fn rule_watch_path_outside_configured_targets_is_rejected() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.watch_path = path_string(&fixture.root.join("outside"));

        let error = validate_rule(&rule, &config).expect_err("outside watch path should fail");

        assert_eq!(error.code, "PATH_OUT_OF_SCOPE");
    }

    #[test]
    fn move_rule_destination_with_parent_escape_is_rejected() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let mut rule = fixture.rule();
        rule.action = RuleAction::Move {
            destination_path: path_string(
                &fixture.root.join("safe").join("..").join("outside.txt"),
            ),
        };

        let error = validate_rule(&rule, &config).expect_err("escaped destination should fail");

        assert_eq!(error.code, "RULE_INVALID_DESTINATION");
    }

    struct Fixture {
        root: PathBuf,
        watch: PathBuf,
        db: std::sync::Arc<redb::Database>,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("shelflife-rule-{}", Uuid::new_v4()));
            let watch = root.join("watch");
            fs::create_dir_all(&watch).expect("watch directory should be created");
            let db = storage::open_database(root.join("test.redb")).expect("database should open");
            Self { root, watch, db }
        }

        fn save_config(&self) {
            storage::save_config(&self.db, &self.config()).expect("config should save");
        }

        fn config(&self) -> AppConfig {
            AppConfig {
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
                safe_folder_path: path_string(&self.root.join("safe")),
                ..AppConfig::default()
            }
        }

        fn rule(&self) -> AutomationRule {
            AutomationRule {
                id: String::from("zip-rule"),
                name: String::from("Zip downloads"),
                enabled: true,
                priority: 10,
                watch_path: path_string(&self.watch),
                ttl_seconds: 86_400,
                conditions: RuleConditions {
                    extensions: vec![String::from("zip")],
                    filename_globs: Vec::new(),
                    filename_regexes: Vec::new(),
                    source_domains: Vec::new(),
                    size: SizeCondition::Any,
                },
                action: RuleAction::Trash,
                mode: RuleMode::PreviewOnly,
                created_at: 1,
                updated_at: 1,
            }
        }

        fn write_watch_file(&self, name: &str, content: &str) -> PathBuf {
            let path = self.watch.join(name);
            fs::write(&path, content).expect("test file should be written");
            path
        }

        fn track_file(&self, path: &Path) {
            let metadata = fs::metadata(path).expect("metadata should exist");
            let mut tracked =
                tracked_file_from_metadata(path, &metadata, None, &AppConfig::default());
            tracked.origin = OriginEvidence::Unknown;
            storage::tracked::upsert_tracked_file(&self.db, &tracked)
                .expect("tracked file should save");
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
