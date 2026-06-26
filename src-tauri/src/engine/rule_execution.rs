use redb::Database;
use std::collections::HashSet;
use std::time::Duration;
use uuid::Uuid;

use crate::engine::automatic_rule_candidate;
use crate::models::{
    AppError, AuditActionKind, AuditEntry, AutomationRule, RuleAction, RuleMatchExplanation,
    TrackedFile, UndoStatus,
};
use crate::storage;

#[derive(Debug, Clone)]
pub struct RuleExecutionReport {
    pub entries: Vec<AuditEntry>,
    pub failures: Vec<AppError>,
}

pub fn execute_expired_automatic_rules(db: &Database) -> Result<RuleExecutionReport, AppError> {
    let config = storage::get_config(db)?;
    let rules = storage::rules::list_rules(db)?;
    let now = crate::engine::freshness::now_seconds();
    let files = storage::tracked::list_tracked_files(db)?;
    let mut failed_attempts = failed_automatic_rule_attempts(db)?;

    let mut entries = Vec::new();
    let mut failures = Vec::new();

    for file in files {
        let Some(candidate) = automatic_rule_candidate(&file, &config, &rules)? else {
            continue;
        };
        if candidate.expires_at > now {
            continue;
        }
        if failed_attempts.contains(&(file.path.clone(), candidate.rule.id.clone())) {
            continue;
        }

        match crate::engine::executor::execute_automation_rule_action(
            db,
            &file.path,
            &candidate.rule,
            candidate.explanation.clone(),
        ) {
            Ok(entry) => {
                entries.push(entry);
            }
            Err(error) => {
                let failure_entry = append_failed_rule_execution_audit_entry(
                    db,
                    &file,
                    &candidate.rule,
                    candidate.explanation,
                    &error,
                )?;
                entries.push(failure_entry);
                failures.push(error);
                failed_attempts.insert((file.path.clone(), candidate.rule.id));
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
    let rules = storage::rules::list_rules(db)?;
    let now = crate::engine::freshness::now_seconds();
    let mut nearest_expiry: Option<u64> = None;
    let failed_attempts = failed_automatic_rule_attempts(db)?;

    for file in storage::tracked::list_tracked_files(db)? {
        let Some(candidate) = automatic_rule_candidate(&file, &config, &rules)? else {
            continue;
        };
        if failed_attempts.contains(&(file.path.clone(), candidate.rule.id.clone())) {
            continue;
        }
        nearest_expiry = Some(match nearest_expiry {
            Some(existing) => existing.min(candidate.expires_at),
            None => candidate.expires_at,
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
    storage::audit::append_audit_entry(db, &entry)?;
    Ok(entry)
}

fn failed_automatic_rule_attempts(db: &Database) -> Result<HashSet<(String, String)>, AppError> {
    Ok(storage::audit::list_audit_entries(db)?
        .into_iter()
        .filter(|entry| matches!(entry.undo_status, UndoStatus::Failed { .. }))
        .filter_map(|entry| {
            let rule_id = entry.rule_id?;
            Some((entry.source_path, rule_id))
        })
        .collect())
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
    use crate::models::{AuditActionKind, Expiry, RuleAction, RuleMode, UndoStatus};
    use crate::storage;
    use crate::storage::test_util::{path_string, Fixture};

    use super::execute_expired_automatic_rules;

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
                .is_some()
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
    fn failed_automatic_rule_records_failed_audit_entry() {
        let fixture = Fixture::new("shelflife-rule-execution");
        fixture.save_config();
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.track_file(&file);
        expire_tracked_file(&fixture, &file);

        let mut rule = fixture.rule();
        rule.mode = RuleMode::Automatic;
        rule.action = RuleAction::Move {
            destination_folder: path_string(&fixture.watch),
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
            storage::audit::list_audit_entries(&fixture.db)
                .expect("audit list should work")
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
