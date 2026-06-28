use std::collections::HashSet;

use rusqlite::{params, OptionalExtension, Row, Transaction};

use crate::models::{AppError, AuditActionKind, AuditEntry, RuleMatchExplanation, UndoStatus};
use crate::storage::{
    i64_to_u64, opt_i64_to_u64, opt_u64_to_i64, rule_action_from_parts, rule_action_parts,
    rule_mode_from_label, rule_mode_label, storage_data_error, u64_to_i64, Database,
};

pub fn next_audit_sequence(db: &Database) -> Result<u64, AppError> {
    db.write(|tx| {
        let next = match tx
            .query_row(
                "SELECT next_sequence FROM audit_sequence_state WHERE id = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        {
            Some(next) => next,
            None => tx.query_row(
                "SELECT COALESCE(MAX(sequence) + 1, 1) FROM audit_entries",
                [],
                |row| row.get::<_, i64>(0),
            )?,
        };
        let following = next.checked_add(1).ok_or_else(|| {
            storage_data_error("Audit sequence counter overflowed.", format!("{next}"))
        })?;
        tx.execute(
            "
            INSERT INTO audit_sequence_state (id, next_sequence)
            VALUES (1, ?1)
            ON CONFLICT(id) DO UPDATE SET next_sequence = excluded.next_sequence
            ",
            params![following],
        )?;
        i64_to_u64(next, "audit_sequence_state.next_sequence")
    })
}

pub fn get_audit_entry_by_id(db: &Database, id: &str) -> Result<Option<AuditEntry>, AppError> {
    let conn = db.connect()?;
    let row = conn
        .query_row(
            audit_select_sql("WHERE id = ?1").as_str(),
            params![id],
            audit_row_from_sql,
        )
        .optional()?;

    row.map(audit_entry_from_row).transpose()
}

pub fn list_audit_entries(db: &Database) -> Result<Vec<AuditEntry>, AppError> {
    let conn = db.connect()?;
    let mut stmt = conn.prepare(audit_select_sql("ORDER BY sequence DESC").as_str())?;
    let rows = stmt.query_map([], audit_row_from_sql)?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(audit_entry_from_row(row?)?);
    }
    Ok(entries)
}

pub fn list_failed_automatic_rule_attempts(
    db: &Database,
) -> Result<HashSet<(String, String)>, AppError> {
    let conn = db.connect()?;
    let mut stmt = conn.prepare(
        "
        SELECT source_path, rule_id
        FROM audit_entries
        WHERE undo_status_kind = 'failed'
          AND rule_id IS NOT NULL
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut attempts = HashSet::new();
    for row in rows {
        attempts.insert(row?);
    }
    Ok(attempts)
}

pub fn append_audit_entry(db: &Database, entry: &AuditEntry) -> Result<(), AppError> {
    db.write(|tx| {
        upsert_audit_entry_tx(tx, entry)?;
        Ok(())
    })
}

pub fn update_audit_entry(db: &Database, entry: &AuditEntry) -> Result<(), AppError> {
    db.write(|tx| {
        upsert_audit_entry_tx(tx, entry)?;
        Ok(())
    })
}

fn upsert_audit_entry_tx(tx: &Transaction<'_>, entry: &AuditEntry) -> Result<(), AppError> {
    let (undo_status_kind, undo_status_reason) = undo_status_parts(&entry.undo_status);
    let explanation = entry.explanation.as_ref();
    let (proposed_action_kind, proposed_action_destination_folder, proposed_action_rename_template) =
        match explanation.and_then(|explanation| explanation.proposed_action.as_ref()) {
            Some(action) => {
                let (kind, destination_folder, rename_template) = rule_action_parts(action);
                (Some(kind), destination_folder, rename_template)
            }
            None => (None, None, None),
        };
    let explanation_size_bytes = explanation
        .map(|explanation| {
            opt_u64_to_i64(
                explanation.size_bytes,
                "audit_entries.explanation_size_bytes",
            )
        })
        .transpose()?
        .flatten();

    tx.execute(
        "
        INSERT INTO audit_entries (
            sequence,
            id,
            timestamp,
            action_kind,
            source_path,
            destination_path,
            file_name,
            size_bytes,
            rule_id,
            rule_name,
            undo_status_kind,
            undo_status_reason,
            explanation_file_path,
            explanation_size_bytes,
            explanation_rule_id,
            explanation_rule_name,
            explanation_matched_extension,
            explanation_matched_size,
            explanation_matched_origin,
            explanation_matched_filename_pattern,
            explanation_proposed_action_kind,
            explanation_proposed_action_destination_folder,
            explanation_proposed_action_rename_template,
            explanation_mode,
            explanation_message
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)
        ON CONFLICT(sequence) DO UPDATE SET
            id = excluded.id,
            timestamp = excluded.timestamp,
            action_kind = excluded.action_kind,
            source_path = excluded.source_path,
            destination_path = excluded.destination_path,
            file_name = excluded.file_name,
            size_bytes = excluded.size_bytes,
            rule_id = excluded.rule_id,
            rule_name = excluded.rule_name,
            undo_status_kind = excluded.undo_status_kind,
            undo_status_reason = excluded.undo_status_reason,
            explanation_file_path = excluded.explanation_file_path,
            explanation_size_bytes = excluded.explanation_size_bytes,
            explanation_rule_id = excluded.explanation_rule_id,
            explanation_rule_name = excluded.explanation_rule_name,
            explanation_matched_extension = excluded.explanation_matched_extension,
            explanation_matched_size = excluded.explanation_matched_size,
            explanation_matched_origin = excluded.explanation_matched_origin,
            explanation_matched_filename_pattern = excluded.explanation_matched_filename_pattern,
            explanation_proposed_action_kind = excluded.explanation_proposed_action_kind,
            explanation_proposed_action_destination_folder = excluded.explanation_proposed_action_destination_folder,
            explanation_proposed_action_rename_template = excluded.explanation_proposed_action_rename_template,
            explanation_mode = excluded.explanation_mode,
            explanation_message = excluded.explanation_message
        ",
        params![
            u64_to_i64(entry.sequence, "audit_entries.sequence")?,
            &entry.id,
            u64_to_i64(entry.timestamp, "audit_entries.timestamp")?,
            audit_action_kind_label(&entry.action_kind),
            &entry.source_path,
            &entry.destination_path,
            &entry.file_name,
            u64_to_i64(entry.size_bytes, "audit_entries.size_bytes")?,
            &entry.rule_id,
            &entry.rule_name,
            undo_status_kind,
            undo_status_reason,
            explanation.map(|explanation| explanation.file_path.as_str()),
            explanation_size_bytes,
            explanation.and_then(|explanation| explanation.rule_id.as_deref()),
            explanation.and_then(|explanation| explanation.rule_name.as_deref()),
            explanation.map(|explanation| explanation.matched_extension),
            explanation.map(|explanation| explanation.matched_size),
            explanation.and_then(|explanation| explanation.matched_origin.as_deref()),
            explanation.and_then(|explanation| explanation.matched_filename_pattern.as_deref()),
            proposed_action_kind,
            proposed_action_destination_folder,
            proposed_action_rename_template,
            explanation.and_then(|explanation| explanation.mode.as_ref().map(rule_mode_label)),
            explanation.map(|explanation| explanation.message.as_str()),
        ],
    )?;

    Ok(())
}

fn audit_entry_from_row(row: AuditRow) -> Result<AuditEntry, AppError> {
    let sequence = i64_to_u64(row.sequence, "audit_entries.sequence")?;
    let explanation = explanation_from_row(row.explanation)?;
    Ok(AuditEntry {
        id: row.id,
        sequence,
        timestamp: i64_to_u64(row.timestamp, "audit_entries.timestamp")?,
        action_kind: audit_action_kind_from_label(&row.action_kind)?,
        source_path: row.source_path,
        destination_path: row.destination_path,
        file_name: row.file_name,
        size_bytes: i64_to_u64(row.size_bytes, "audit_entries.size_bytes")?,
        rule_id: row.rule_id,
        rule_name: row.rule_name,
        explanation,
        undo_status: undo_status_from_parts(&row.undo_status_kind, row.undo_status_reason)?,
    })
}

fn explanation_from_row(row: ExplanationRow) -> Result<Option<RuleMatchExplanation>, AppError> {
    let Some(message) = row.message else {
        return Ok(None);
    };
    let proposed_action = row
        .proposed_action_kind
        .map(|kind| {
            rule_action_from_parts(
                &kind,
                row.proposed_action_destination_folder,
                row.proposed_action_rename_template,
            )
        })
        .transpose()?;
    let mode = row
        .mode
        .map(|mode| rule_mode_from_label(&mode))
        .transpose()?;

    Ok(Some(RuleMatchExplanation {
        file_path: row.file_path.ok_or_else(|| {
            storage_data_error(
                "Stored audit explanation is missing its file path.",
                message.clone(),
            )
        })?,
        size_bytes: opt_i64_to_u64(row.size_bytes, "audit_entries.explanation_size_bytes")?,
        rule_id: row.rule_id,
        rule_name: row.rule_name,
        matched_extension: row.matched_extension.ok_or_else(|| {
            storage_data_error(
                "Stored audit explanation is missing its extension match flag.",
                message.clone(),
            )
        })?,
        matched_size: row.matched_size.ok_or_else(|| {
            storage_data_error(
                "Stored audit explanation is missing its size match flag.",
                message.clone(),
            )
        })?,
        matched_origin: row.matched_origin,
        matched_filename_pattern: row.matched_filename_pattern,
        proposed_action,
        mode,
        message,
    }))
}

fn audit_select_sql(predicate: &str) -> String {
    format!(
        "
        SELECT sequence,
               id,
               timestamp,
               action_kind,
               source_path,
               destination_path,
               file_name,
               size_bytes,
               rule_id,
               rule_name,
               undo_status_kind,
               undo_status_reason,
               explanation_file_path,
               explanation_size_bytes,
               explanation_rule_id,
               explanation_rule_name,
               explanation_matched_extension,
               explanation_matched_size,
               explanation_matched_origin,
               explanation_matched_filename_pattern,
               explanation_proposed_action_kind,
               explanation_proposed_action_destination_folder,
               explanation_proposed_action_rename_template,
               explanation_mode,
               explanation_message
        FROM audit_entries
        {predicate}
        "
    )
}

fn audit_row_from_sql(row: &Row<'_>) -> rusqlite::Result<AuditRow> {
    Ok(AuditRow {
        sequence: row.get(0)?,
        id: row.get(1)?,
        timestamp: row.get(2)?,
        action_kind: row.get(3)?,
        source_path: row.get(4)?,
        destination_path: row.get(5)?,
        file_name: row.get(6)?,
        size_bytes: row.get(7)?,
        rule_id: row.get(8)?,
        rule_name: row.get(9)?,
        undo_status_kind: row.get(10)?,
        undo_status_reason: row.get(11)?,
        explanation: ExplanationRow {
            file_path: row.get(12)?,
            size_bytes: row.get(13)?,
            rule_id: row.get(14)?,
            rule_name: row.get(15)?,
            matched_extension: row.get(16)?,
            matched_size: row.get(17)?,
            matched_origin: row.get(18)?,
            matched_filename_pattern: row.get(19)?,
            proposed_action_kind: row.get(20)?,
            proposed_action_destination_folder: row.get(21)?,
            proposed_action_rename_template: row.get(22)?,
            mode: row.get(23)?,
            message: row.get(24)?,
        },
    })
}

struct AuditRow {
    sequence: i64,
    id: String,
    timestamp: i64,
    action_kind: String,
    source_path: String,
    destination_path: Option<String>,
    file_name: String,
    size_bytes: i64,
    rule_id: Option<String>,
    rule_name: Option<String>,
    undo_status_kind: String,
    undo_status_reason: Option<String>,
    explanation: ExplanationRow,
}

struct ExplanationRow {
    file_path: Option<String>,
    size_bytes: Option<i64>,
    rule_id: Option<String>,
    rule_name: Option<String>,
    matched_extension: Option<bool>,
    matched_size: Option<bool>,
    matched_origin: Option<String>,
    matched_filename_pattern: Option<String>,
    proposed_action_kind: Option<String>,
    proposed_action_destination_folder: Option<String>,
    proposed_action_rename_template: Option<String>,
    mode: Option<String>,
    message: Option<String>,
}

fn audit_action_kind_label(kind: &AuditActionKind) -> &'static str {
    match kind {
        AuditActionKind::Trash => "trash",
        AuditActionKind::Move => "move",
        AuditActionKind::Pin => "pin",
        AuditActionKind::Snooze => "snooze",
        AuditActionKind::Ignore => "ignore",
        AuditActionKind::RulePreview => "rule_preview",
    }
}

fn audit_action_kind_from_label(value: &str) -> Result<AuditActionKind, AppError> {
    match value {
        "trash" => Ok(AuditActionKind::Trash),
        "move" => Ok(AuditActionKind::Move),
        "pin" => Ok(AuditActionKind::Pin),
        "snooze" => Ok(AuditActionKind::Snooze),
        "ignore" => Ok(AuditActionKind::Ignore),
        "rule_preview" => Ok(AuditActionKind::RulePreview),
        other => Err(storage_data_error(
            "Stored audit action kind is not recognized.",
            other,
        )),
    }
}

fn undo_status_parts(status: &UndoStatus) -> (&'static str, Option<&str>) {
    match status {
        UndoStatus::Available => ("available", None),
        UndoStatus::Unavailable { reason } => ("unavailable", Some(reason.as_str())),
        UndoStatus::Completed => ("completed", None),
        UndoStatus::Failed { reason } => ("failed", Some(reason.as_str())),
    }
}

fn undo_status_from_parts(kind: &str, reason: Option<String>) -> Result<UndoStatus, AppError> {
    match kind {
        "available" => Ok(UndoStatus::Available),
        "unavailable" => Ok(UndoStatus::Unavailable {
            reason: reason.unwrap_or_default(),
        }),
        "completed" => Ok(UndoStatus::Completed),
        "failed" => Ok(UndoStatus::Failed {
            reason: reason.unwrap_or_default(),
        }),
        other => Err(storage_data_error(
            "Stored undo status is not recognized.",
            other,
        )),
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::models::{
        AuditActionKind, AuditEntry, RuleAction, RuleMatchExplanation, RuleMode, UndoStatus,
    };
    use crate::storage::test_util::{path_string, Fixture};

    #[test]
    fn audit_entries_round_trip_explanations_and_undo_status() {
        let fixture = Fixture::new("shelflife-audit-round-trip");
        let file = fixture.write_watch_file("download.zip", "body");
        let entry = AuditEntry {
            id: Uuid::new_v4().to_string(),
            sequence: super::next_audit_sequence(&fixture.db).expect("sequence should allocate"),
            timestamp: 42,
            action_kind: AuditActionKind::Move,
            source_path: path_string(&file),
            destination_path: Some(path_string(&fixture.safe.join("download.zip"))),
            file_name: String::from("download.zip"),
            size_bytes: 4,
            rule_id: Some(String::from("zip-rule")),
            rule_name: Some(String::from("Zip downloads")),
            explanation: Some(RuleMatchExplanation {
                file_path: path_string(&file),
                size_bytes: Some(4),
                rule_id: Some(String::from("zip-rule")),
                rule_name: Some(String::from("Zip downloads")),
                matched_extension: true,
                matched_size: true,
                matched_origin: Some(String::from("example.com")),
                matched_filename_pattern: Some(String::from("*.zip")),
                proposed_action: Some(RuleAction::Move {
                    destination_folder: path_string(&fixture.safe),
                    rename_template: Some(String::from("{name}.{ext}")),
                }),
                mode: Some(RuleMode::Automatic),
                message: String::from("Matched rule."),
            }),
            undo_status: UndoStatus::Unavailable {
                reason: String::from("test"),
            },
        };

        super::append_audit_entry(&fixture.db, &entry).expect("audit entry should save");

        let loaded = super::get_audit_entry_by_id(&fixture.db, &entry.id)
            .expect("audit lookup should work")
            .expect("audit entry should exist");
        assert_eq!(loaded, entry);
    }

    #[test]
    fn next_audit_sequence_is_monotonic() {
        let fixture = Fixture::new("shelflife-audit-sequence");

        let first =
            super::next_audit_sequence(&fixture.db).expect("first sequence should allocate");
        let second =
            super::next_audit_sequence(&fixture.db).expect("second sequence should allocate");

        assert_eq!(first, 1);
        assert_eq!(second, 2);
    }
}
