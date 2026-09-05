use std::time::Duration;
use uuid::Uuid;

use crate::engine::{automatic_rule_candidate, AutomaticRuleCandidate};
use crate::models::{
    AppError, AuditActionKind, AuditEntry, AutomationRule, RuleAction, RuleMatchExplanation,
    TrackedFile, UndoStatus,
};
use crate::rules::CompiledRuleSet;
use crate::storage::{self, Database};

#[derive(Debug, Clone)]
pub struct RuleExecutionReport {
    pub entries: Vec<AuditEntry>,
    pub failures: Vec<AppError>,
}

pub fn execute_expired_automatic_rules(db: &Database) -> Result<RuleExecutionReport, AppError> {
    let config = storage::get_config(db)?;
    let rule_set = CompiledRuleSet::compile(storage::rules::list_rules(db)?, &config)?;
    let now = crate::engine::freshness::now_seconds();
    let files = storage::tracked::list_tracked_files(db)?;
    let mut candidates = Vec::new();

    for file in files {
        let Some(candidate) = automatic_rule_candidate(&file, &config, &rule_set) else {
            continue;
        };
        let Some(eligible_at) = candidate.eligible_at else {
            continue;
        };
        if eligible_at > now {
            continue;
        }
        candidates.push((file, candidate.rule, candidate.explanation));
    }

    execute_automatic_rule_candidates(db, candidates)
}

pub fn execute_arrival_automatic_rules(
    db: &Database,
    candidates: &[AutomaticRuleCandidate],
) -> Result<RuleExecutionReport, AppError> {
    let mut executions = Vec::with_capacity(candidates.len());

    for candidate in candidates {
        let Some(file) = storage::tracked::get_tracked_file(db, &candidate.file_path)? else {
            continue;
        };
        executions.push((file, candidate.rule.clone(), candidate.explanation.clone()));
    }

    execute_automatic_rule_candidates(db, executions)
}

fn execute_automatic_rule_candidates(
    db: &Database,
    candidates: impl IntoIterator<Item = (TrackedFile, AutomationRule, RuleMatchExplanation)>,
) -> Result<RuleExecutionReport, AppError> {
    let mut failed_attempts = storage::audit::list_failed_automatic_rule_attempts(db)?;
    let mut entries = Vec::new();
    let mut failures = Vec::new();

    for (file, rule, explanation) in candidates {
        if failed_attempts.contains(&(file.path.clone(), rule.id.clone())) {
            continue;
        }

        match crate::engine::executor::execute_automation_rule_action(
            db,
            &file.path,
            &rule,
            explanation.clone(),
        ) {
            Ok(entry) => entries.push(entry),
            Err(failure) => {
                let error = failure.error;
                let failure_entry = match failure.audit_entry {
                    Some(entry) => *entry,
                    None => append_failed_rule_execution_audit_entry(
                        db,
                        &file,
                        &rule,
                        explanation,
                        &error,
                    )?,
                };
                entries.push(failure_entry);
                failures.push(error);
                failed_attempts.insert((file.path, rule.id));
            }
        }
    }

    Ok(RuleExecutionReport { entries, failures })
}

pub fn next_automatic_rule_execution_delay(
    db: &Database,
    minimum_interval: Duration,
) -> Result<Option<Duration>, AppError> {
    let config = storage::get_config(db)?;
    let rule_set = CompiledRuleSet::compile(storage::rules::list_rules(db)?, &config)?;
    let now = crate::engine::freshness::now_seconds();
    let mut nearest_expiry: Option<u64> = None;
    let failed_attempts = storage::audit::list_failed_automatic_rule_attempts(db)?;

    for file in storage::tracked::list_tracked_files(db)? {
        let Some(candidate) = automatic_rule_candidate(&file, &config, &rule_set) else {
            continue;
        };
        if failed_attempts.contains(&(file.path.clone(), candidate.rule.id.clone())) {
            continue;
        }
        let Some(eligible_at) = candidate.eligible_at else {
            continue;
        };
        nearest_expiry = Some(match nearest_expiry {
            Some(existing) => existing.min(eligible_at),
            None => eligible_at,
        });
    }

    let Some(expires_at) = nearest_expiry else {
        return Ok(None);
    };

    if expires_at <= now {
        return Ok(Some(minimum_interval));
    }

    Ok(Some(
        Duration::from_secs(expires_at - now).max(minimum_interval),
    ))
}

fn append_failed_rule_execution_audit_entry(
    db: &Database,
    file: &TrackedFile,
    rule: &AutomationRule,
    explanation: RuleMatchExplanation,
    error: &AppError,
) -> Result<AuditEntry, AppError> {
    let reason = match &error.details {
        Some(details) => format!("{} Details: {}", error.message, details),
        None => error.message.clone(),
    };
    let entry = AuditEntry {
        id: Uuid::new_v4().to_string(),
        sequence: storage::audit::next_audit_sequence(db)?,
        timestamp: crate::engine::freshness::now_seconds(),
        action_kind: audit_action_kind_for_rule_action(&rule.action),
        source_path: file.path.clone(),
        destination_path: None,
        file_name: file.file_name.clone(),
        size_bytes: file.size_bytes,
        rule_id: Some(rule.id.clone()),
        rule_name: Some(rule.name.clone()),
        explanation: Some(explanation),
        undo_status: UndoStatus::Failed { reason },
    };
    storage::audit::upsert_audit_entry(db, &entry)?;
    Ok(entry)
}

fn audit_action_kind_for_rule_action(action: &RuleAction) -> AuditActionKind {
    match action {
        RuleAction::Trash => AuditActionKind::Trash,
        RuleAction::Move { .. } => AuditActionKind::Move,
        RuleAction::Ignore => AuditActionKind::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{AuditActionKind, Expiry, RuleAction, RuleMode, RuleTiming, UndoStatus};
    use crate::storage;
    use crate::storage::test_util::{path_string, Fixture};

    use super::{execute_arrival_automatic_rules, execute_expired_automatic_rules};

    #[test]
    fn arrival_move_rule_executes_only_for_newly_indexed_paths() {
        let fixture = Fixture::new("shelflife-arrival-rule-execution");
        fixture.save_config();
        let arriving = fixture.write_watch_file("arriving.zip", "body");
        let existing = fixture.write_watch_file("existing.zip", "body");
        fixture.track_file(&existing);

        let mut rule = fixture.rule();
        rule.mode = RuleMode::Automatic;
        rule.timing = RuleTiming::OnArrival;
        rule.action = RuleAction::Move {
            destination_folder: path_string(&fixture.outside),
            rename_template: None,
        };
        storage::rules::save_rule(&fixture.db, &rule).expect("rule should save");

        let (reconciliation_report, arrival_candidates) =
            crate::engine::reconciliation::reconcile_paths(&fixture.db, vec![arriving.clone()])
                .expect("incremental reconciliation should succeed");
        assert_eq!(reconciliation_report.indexed, vec![path_string(&arriving)]);
        assert_eq!(arrival_candidates.len(), 1);

        let report = execute_arrival_automatic_rules(&fixture.db, &arrival_candidates)
            .expect("arrival rule execution should run");

        assert!(report.failures.is_empty());
        assert_eq!(report.entries.len(), 1);
        assert!(!arriving.exists());
        assert!(fixture.outside.join("arriving.zip").exists());
        assert!(existing.exists());

        expire_tracked_file(&fixture, &existing);
        let expiry_report =
            execute_expired_automatic_rules(&fixture.db).expect("expiry execution should run");
        assert!(expiry_report.entries.is_empty());
        assert!(existing.exists());
    }

    #[test]
    fn expired_automatic_move_rule_executes_and_records_rule_metadata() {
        let fixture = Fixture::new("shelflife-rule-execution");
        fixture.save_config();
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.track_file(&file);
        expire_tracked_file(&fixture, &file);

        let mut rule = fixture.rule();
        rule.mode = RuleMode::Automatic;
        rule.action = RuleAction::Move {
            destination_folder: path_string(&fixture.outside),
            rename_template: Some(String::from("archived-{name}.{ext}")),
        };
        storage::rules::save_rule(&fixture.db, &rule).expect("rule should save");

        let report =
            execute_expired_automatic_rules(&fixture.db).expect("rule execution should run");

        let destination = fixture.outside.join("archived-download.zip");
        assert!(report.failures.is_empty());
        assert_eq!(report.entries.len(), 1);
        assert!(!file.exists());
        assert!(destination.exists());
        assert_eq!(report.entries[0].action_kind, AuditActionKind::Move);
        assert_eq!(report.entries[0].rule_id, Some(String::from("zip-rule")));
        assert_eq!(
            report.entries[0].destination_path,
            Some(path_string(&destination))
        );
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&destination))
                .expect("tracked lookup should work")
                .is_none()
        );
    }

    #[test]
    fn expired_ask_first_rule_does_not_execute_without_user_action() {
        let fixture = Fixture::new("shelflife-rule-execution");
        fixture.save_config();
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.track_file(&file);
        expire_tracked_file(&fixture, &file);

        let mut rule = fixture.rule();
        rule.mode = RuleMode::AskFirst;
        rule.action = RuleAction::Move {
            destination_folder: path_string(&fixture.outside),
            rename_template: None,
        };
        storage::rules::save_rule(&fixture.db, &rule).expect("rule should save");

        let report =
            execute_expired_automatic_rules(&fixture.db).expect("rule execution should run");

        assert!(report.entries.is_empty());
        assert!(report.failures.is_empty());
        assert!(file.exists());
    }

    #[test]
    fn higher_priority_ask_first_rule_blocks_lower_priority_automatic_rule() {
        let fixture = Fixture::new("shelflife-rule-execution");
        fixture.save_config();
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.track_file(&file);
        expire_tracked_file(&fixture, &file);

        let mut ask_first_rule = fixture.rule();
        ask_first_rule.id = String::from("ask-first-zip-rule");
        ask_first_rule.name = String::from("Review zip downloads");
        ask_first_rule.priority = 20;
        ask_first_rule.mode = RuleMode::AskFirst;
        storage::rules::save_rule(&fixture.db, &ask_first_rule).expect("rule should save");

        let mut automatic_rule = fixture.rule();
        automatic_rule.id = String::from("auto-zip-rule");
        automatic_rule.name = String::from("Archive zip downloads");
        automatic_rule.priority = 10;
        automatic_rule.mode = RuleMode::Automatic;
        automatic_rule.action = RuleAction::Move {
            destination_folder: path_string(&fixture.outside),
            rename_template: None,
        };
        storage::rules::save_rule(&fixture.db, &automatic_rule).expect("rule should save");

        let report =
            execute_expired_automatic_rules(&fixture.db).expect("rule execution should run");
        let delay = super::next_automatic_rule_execution_delay(
            &fixture.db,
            std::time::Duration::from_secs(5),
        )
        .expect("delay should calculate");

        assert!(report.entries.is_empty());
        assert!(report.failures.is_empty());
        assert!(delay.is_none());
        assert!(file.exists());
    }

    #[test]
    fn higher_priority_preview_rule_does_not_block_lower_priority_automatic_rule() {
        let fixture = Fixture::new("shelflife-rule-execution");
        fixture.save_config();
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.track_file(&file);
        expire_tracked_file(&fixture, &file);

        let mut preview_rule = fixture.rule();
        preview_rule.id = String::from("preview-zip-rule");
        preview_rule.name = String::from("Preview zip downloads");
        preview_rule.priority = 20;
        preview_rule.mode = RuleMode::PreviewOnly;
        storage::rules::save_rule(&fixture.db, &preview_rule).expect("rule should save");

        let mut automatic_rule = fixture.rule();
        automatic_rule.id = String::from("auto-zip-rule");
        automatic_rule.name = String::from("Archive zip downloads");
        automatic_rule.priority = 10;
        automatic_rule.mode = RuleMode::Automatic;
        automatic_rule.action = RuleAction::Move {
            destination_folder: path_string(&fixture.outside),
            rename_template: None,
        };
        storage::rules::save_rule(&fixture.db, &automatic_rule).expect("rule should save");

        let report =
            execute_expired_automatic_rules(&fixture.db).expect("rule execution should run");

        assert!(report.failures.is_empty());
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].rule_id, Some(automatic_rule.id));
        assert!(!file.exists());
        assert!(fixture.outside.join("download.zip").exists());
    }

    #[test]
    fn failed_automatic_rule_records_failed_audit_entry() {
        let fixture = Fixture::new("shelflife-rule-execution");
        fixture.save_config();
        let file = fixture.write_watch_file("download.zip", "body");
        let blocked_destination = fixture.write_outside_file("not-a-folder", "blocking file");
        fixture.track_file(&file);
        expire_tracked_file(&fixture, &file);

        let mut rule = fixture.rule();
        rule.mode = RuleMode::Automatic;
        rule.action = RuleAction::Move {
            destination_folder: path_string(&blocked_destination),
            rename_template: None,
        };
        storage::rules::save_rule(&fixture.db, &rule).expect("rule should save");

        let report =
            execute_expired_automatic_rules(&fixture.db).expect("rule execution should run");

        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.entries.len(), 1);
        assert!(file.exists());
        assert_eq!(report.entries[0].action_kind, AuditActionKind::Move);
        assert_eq!(report.entries[0].rule_id, Some(String::from("zip-rule")));
        assert!(matches!(
            report.entries[0].undo_status,
            UndoStatus::Failed { .. }
        ));
        assert_eq!(
            storage::audit::list_audit_entries_page(&fixture.db, None, "")
                .expect("audit list should work")
                .entries
                .len(),
            1
        );
        let second_report =
            execute_expired_automatic_rules(&fixture.db).expect("rule execution should run again");
        let delay = super::next_automatic_rule_execution_delay(
            &fixture.db,
            std::time::Duration::from_secs(5),
        )
        .expect("delay should calculate");

        assert!(second_report.entries.is_empty());
        assert!(second_report.failures.is_empty());
        assert!(delay.is_none());
    }

    #[test]
    fn automatic_ignore_rule_is_not_scheduled_as_expired_action() {
        let fixture = Fixture::new("shelflife-rule-execution");
        fixture.save_config();
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.track_file(&file);
        expire_tracked_file(&fixture, &file);

        let mut rule = fixture.rule();
        rule.mode = RuleMode::Automatic;
        rule.action = RuleAction::Ignore;
        storage::rules::save_rule(&fixture.db, &rule).expect("rule should save");

        let report =
            execute_expired_automatic_rules(&fixture.db).expect("rule execution should run");
        let delay = super::next_automatic_rule_execution_delay(
            &fixture.db,
            std::time::Duration::from_secs(5),
        )
        .expect("delay should calculate");

        assert!(report.entries.is_empty());
        assert!(report.failures.is_empty());
        assert!(delay.is_none());
        assert!(file.exists());
    }

    #[test]
    fn next_execution_delay_uses_nearest_automatic_rule_expiry() {
        let fixture = Fixture::new("shelflife-rule-execution");
        fixture.save_config();
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.track_file(&file);

        let mut tracked = storage::tracked::get_tracked_file(&fixture.db, &path_string(&file))
            .expect("tracked lookup should work")
            .expect("tracked file should exist");
        tracked.expiry = Expiry::At(crate::engine::freshness::now_seconds() + 120);
        storage::tracked::upsert_tracked_file(&fixture.db, &tracked)
            .expect("tracked file should update");

        let mut rule = fixture.rule();
        rule.mode = RuleMode::Automatic;
        storage::rules::save_rule(&fixture.db, &rule).expect("rule should save");

        let delay = super::next_automatic_rule_execution_delay(
            &fixture.db,
            std::time::Duration::from_secs(5),
        )
        .expect("delay should calculate")
        .expect("automatic rule delay should exist");

        assert!(delay <= std::time::Duration::from_secs(120));
        assert!(delay >= std::time::Duration::from_secs(5));
    }

    fn expire_tracked_file(fixture: &Fixture, file: &std::path::Path) {
        let path = path_string(file);
        let mut tracked = storage::tracked::get_tracked_file(&fixture.db, &path)
            .expect("tracked lookup should work")
            .expect("tracked file should exist");
        tracked.expiry = Expiry::At(crate::engine::freshness::now_seconds().saturating_sub(1));
        storage::tracked::upsert_tracked_file(&fixture.db, &tracked)
            .expect("tracked file should update");
    }
}
