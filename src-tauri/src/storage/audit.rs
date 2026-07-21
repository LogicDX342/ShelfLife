use std::collections::HashSet;

use diesel::dsl::max;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::models::{
    AppError, AuditActionKind, AuditEntry, AuditPage, RuleMatchExplanation, UndoStatus,
};
use crate::storage::schema::{audit_entries, audit_sequence_state};
use crate::storage::{
    i64_to_u64, opt_i64_to_u64, rule_action_from_parts, rule_action_parts, rule_mode_from_label,
    rule_mode_label, storage_data_error, u64_to_i64, Database,
};

const AUDIT_PAGE_SIZE: i64 = 50;

macro_rules! audit_search_filter {
    ($pattern:expr) => {
        audit_entries::file_name
            .like($pattern)
            .escape('\\')
            .or(audit_entries::source_path.like($pattern).escape('\\'))
            .or(audit_entries::destination_path.like($pattern).escape('\\'))
            .or(audit_entries::action_kind.like($pattern).escape('\\'))
            .or(audit_entries::rule_name.like($pattern).escape('\\'))
    };
}

pub fn next_audit_sequence(db: &Database) -> Result<u64, AppError> {
    db.write(|conn| {
        let next = match audit_sequence_state::table
            .find(1_i32)
            .select(audit_sequence_state::next_sequence)
            .first::<i64>(conn)
            .optional()?
        {
            Some(next) => next,
            None => {
                let max_sequence = audit_entries::table
                    .select(max(audit_entries::sequence))
                    .first::<Option<i64>>(conn)?;
                match max_sequence {
                    Some(sequence) => sequence.checked_add(1).ok_or_else(|| {
                        storage_data_error(
                            "Audit sequence counter overflowed.",
                            format!("{sequence}"),
                        )
                    })?,
                    None => 1,
                }
            }
        };
        let following = next.checked_add(1).ok_or_else(|| {
            storage_data_error("Audit sequence counter overflowed.", format!("{next}"))
        })?;
        let row = AuditSequenceWriteRow {
            id: 1,
            next_sequence: following,
        };
        diesel::insert_into(audit_sequence_state::table)
            .values(&row)
            .on_conflict(audit_sequence_state::id)
            .do_update()
            .set(audit_sequence_state::next_sequence.eq(following))
            .execute(conn)?;
        i64_to_u64(next, "audit_sequence_state.next_sequence")
    })
}

pub fn get_audit_entry_by_id(db: &Database, id: &str) -> Result<Option<AuditEntry>, AppError> {
    let mut conn = db.connect()?;
    let row = audit_entries::table
        .filter(audit_entries::id.eq(id))
        .select(AuditRow::as_select())
        .first::<AuditRow>(&mut conn)
        .optional()?;

    row.map(audit_entry_from_row).transpose()
}

pub fn list_audit_entries_page(
    db: &Database,
    cursor: Option<u64>,
    search_query: &str,
) -> Result<AuditPage, AppError> {
    let mut conn = db.connect()?;
    let search_pattern = audit_search_pattern(search_query);

    let total_count = if cursor.is_none() {
        let count = match search_pattern.as_deref() {
            Some(pattern) => audit_entries::table
                .filter(audit_search_filter!(pattern))
                .count()
                .get_result::<i64>(&mut conn)?,
            None => audit_entries::table.count().get_result::<i64>(&mut conn)?,
        };
        Some(i64_to_u64(count, "audit page total_count")?)
    } else {
        None
    };

    let mut query = audit_entries::table.into_boxed::<diesel::sqlite::Sqlite>();
    if let Some(cursor) = cursor {
        query = query.filter(audit_entries::sequence.lt(u64_to_i64(cursor, "audit page cursor")?));
    }
    if let Some(pattern) = search_pattern.as_deref() {
        query = query.filter(audit_search_filter!(pattern));
    }

    let mut rows = query
        .order(audit_entries::sequence.desc())
        .limit(AUDIT_PAGE_SIZE + 1)
        .select(AuditRow::as_select())
        .load::<AuditRow>(&mut conn)?;
    let has_more = rows.len() > AUDIT_PAGE_SIZE as usize;
    if has_more {
        rows.pop();
    }

    Ok(AuditPage {
        entries: rows
            .into_iter()
            .map(audit_entry_from_row)
            .collect::<Result<_, _>>()?,
        has_more,
        total_count,
    })
}

fn audit_search_pattern(search_query: &str) -> Option<String> {
    if search_query.is_empty() {
        return None;
    }

    let escaped = search_query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    Some(format!("%{escaped}%"))
}

pub fn list_failed_automatic_rule_attempts(
    db: &Database,
) -> Result<HashSet<(String, String)>, AppError> {
    let mut conn = db.connect()?;
    let rows = audit_entries::table
        .filter(audit_entries::undo_status_kind.eq("failed"))
        .filter(audit_entries::rule_id.is_not_null())
        .select((audit_entries::source_path, audit_entries::rule_id))
        .load::<(String, Option<String>)>(&mut conn)?;
    let mut attempts = HashSet::new();
    for (source_path, rule_id) in rows {
        if let Some(rule_id) = rule_id {
            attempts.insert((source_path, rule_id));
        }
    }
    Ok(attempts)
}

pub fn upsert_audit_entry(db: &Database, entry: &AuditEntry) -> Result<(), AppError> {
    db.write(|conn| {
        upsert_audit_entry_tx(conn, entry)?;
        Ok(())
    })
}

pub(crate) fn upsert_audit_entry_tx(
    conn: &mut SqliteConnection,
    entry: &AuditEntry,
) -> Result<(), AppError> {
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
        .and_then(|explanation| explanation.size_bytes)
        .map(|value| u64_to_i64(value, "audit_entries.explanation_size_bytes"))
        .transpose()?;
    let row = AuditWriteRow {
        sequence: u64_to_i64(entry.sequence, "audit_entries.sequence")?,
        id: &entry.id,
        timestamp: u64_to_i64(entry.timestamp, "audit_entries.timestamp")?,
        action_kind: audit_action_kind_label(&entry.action_kind),
        source_path: &entry.source_path,
        destination_path: entry.destination_path.as_deref(),
        file_name: &entry.file_name,
        size_bytes: u64_to_i64(entry.size_bytes, "audit_entries.size_bytes")?,
        rule_id: entry.rule_id.as_deref(),
        rule_name: entry.rule_name.as_deref(),
        undo_status_kind,
        undo_status_reason,
        explanation_file_path: explanation.map(|explanation| explanation.file_path.as_str()),
        explanation_size_bytes,
        explanation_rule_id: explanation.and_then(|explanation| explanation.rule_id.as_deref()),
        explanation_rule_name: explanation.and_then(|explanation| explanation.rule_name.as_deref()),
        explanation_matched_extension: explanation.map(|explanation| explanation.matched_extension),
        explanation_matched_size: explanation.map(|explanation| explanation.matched_size),
        explanation_matched_origin: explanation
            .and_then(|explanation| explanation.matched_origin.as_deref()),
        explanation_matched_filename_pattern: explanation
            .and_then(|explanation| explanation.matched_filename_pattern.as_deref()),
        explanation_proposed_action_kind: proposed_action_kind,
        explanation_proposed_action_destination_folder: proposed_action_destination_folder,
        explanation_proposed_action_rename_template: proposed_action_rename_template,
        explanation_mode: explanation
            .and_then(|explanation| explanation.mode.as_ref().map(rule_mode_label)),
        explanation_message: explanation.map(|explanation| explanation.message.as_str()),
    };

    diesel::insert_into(audit_entries::table)
        .values(&row)
        .on_conflict(audit_entries::sequence)
        .do_update()
        .set(&row)
        .execute(conn)?;

    Ok(())
}

fn audit_entry_from_row(row: AuditRow) -> Result<AuditEntry, AppError> {
    let sequence = i64_to_u64(row.sequence, "audit_entries.sequence")?;
    let explanation = explanation_from_row(&row)?;
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

fn explanation_from_row(row: &AuditRow) -> Result<Option<RuleMatchExplanation>, AppError> {
    let Some(message) = row.explanation_message.clone() else {
        return Ok(None);
    };
    let proposed_action = row
        .explanation_proposed_action_kind
        .clone()
        .map(|kind| {
            rule_action_from_parts(
                &kind,
                row.explanation_proposed_action_destination_folder.clone(),
                row.explanation_proposed_action_rename_template.clone(),
            )
        })
        .transpose()?;
    let mode = row
        .explanation_mode
        .clone()
        .map(|mode| rule_mode_from_label(&mode))
        .transpose()?;

    Ok(Some(RuleMatchExplanation {
        file_path: row.explanation_file_path.clone().ok_or_else(|| {
            storage_data_error(
                "Stored audit explanation is missing its file path.",
                message.clone(),
            )
        })?,
        size_bytes: opt_i64_to_u64(
            row.explanation_size_bytes,
            "audit_entries.explanation_size_bytes",
        )?,
        rule_id: row.explanation_rule_id.clone(),
        rule_name: row.explanation_rule_name.clone(),
        matched_extension: row.explanation_matched_extension.ok_or_else(|| {
            storage_data_error(
                "Stored audit explanation is missing its extension match flag.",
                message.clone(),
            )
        })?,
        matched_size: row.explanation_matched_size.ok_or_else(|| {
            storage_data_error(
                "Stored audit explanation is missing its size match flag.",
                message.clone(),
            )
        })?,
        matched_origin: row.explanation_matched_origin.clone(),
        matched_filename_pattern: row.explanation_matched_filename_pattern.clone(),
        proposed_action,
        mode,
        message,
    }))
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = audit_entries)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
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
    explanation_file_path: Option<String>,
    explanation_size_bytes: Option<i64>,
    explanation_rule_id: Option<String>,
    explanation_rule_name: Option<String>,
    explanation_matched_extension: Option<bool>,
    explanation_matched_size: Option<bool>,
    explanation_matched_origin: Option<String>,
    explanation_matched_filename_pattern: Option<String>,
    explanation_proposed_action_kind: Option<String>,
    explanation_proposed_action_destination_folder: Option<String>,
    explanation_proposed_action_rename_template: Option<String>,
    explanation_mode: Option<String>,
    explanation_message: Option<String>,
}

#[derive(Insertable, AsChangeset)]
#[diesel(table_name = audit_entries)]
#[diesel(treat_none_as_null = true)]
struct AuditWriteRow<'a> {
    sequence: i64,
    id: &'a str,
    timestamp: i64,
    action_kind: &'a str,
    source_path: &'a str,
    destination_path: Option<&'a str>,
    file_name: &'a str,
    size_bytes: i64,
    rule_id: Option<&'a str>,
    rule_name: Option<&'a str>,
    undo_status_kind: &'a str,
    undo_status_reason: Option<&'a str>,
    explanation_file_path: Option<&'a str>,
    explanation_size_bytes: Option<i64>,
    explanation_rule_id: Option<&'a str>,
    explanation_rule_name: Option<&'a str>,
    explanation_matched_extension: Option<bool>,
    explanation_matched_size: Option<bool>,
    explanation_matched_origin: Option<&'a str>,
    explanation_matched_filename_pattern: Option<&'a str>,
    explanation_proposed_action_kind: Option<&'a str>,
    explanation_proposed_action_destination_folder: Option<&'a str>,
    explanation_proposed_action_rename_template: Option<&'a str>,
    explanation_mode: Option<&'a str>,
    explanation_message: Option<&'a str>,
}

#[derive(Insertable)]
#[diesel(table_name = audit_sequence_state)]
struct AuditSequenceWriteRow {
    id: i32,
    next_sequence: i64,
}

fn audit_action_kind_label(kind: &AuditActionKind) -> &'static str {
    match kind {
        AuditActionKind::Trash => "trash",
        AuditActionKind::Move => "move",
        AuditActionKind::Pin => "pin",
        AuditActionKind::Snooze => "snooze",
        AuditActionKind::Ignore => "ignore",
    }
}

fn audit_action_kind_from_label(value: &str) -> Result<AuditActionKind, AppError> {
    match value {
        "trash" => Ok(AuditActionKind::Trash),
        "move" => Ok(AuditActionKind::Move),
        "pin" => Ok(AuditActionKind::Pin),
        "snooze" => Ok(AuditActionKind::Snooze),
        "ignore" => Ok(AuditActionKind::Ignore),
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

    fn save_audit_entry(
        fixture: &Fixture,
        file_name: &str,
        source_path: Option<String>,
        destination_path: Option<String>,
        action_kind: AuditActionKind,
        rule_name: Option<String>,
    ) -> AuditEntry {
        let entry = AuditEntry {
            id: Uuid::new_v4().to_string(),
            sequence: super::next_audit_sequence(&fixture.db).expect("sequence should allocate"),
            timestamp: 42,
            action_kind,
            source_path: source_path.unwrap_or_else(|| path_string(&fixture.watch.join(file_name))),
            destination_path,
            file_name: file_name.to_string(),
            size_bytes: 4,
            rule_id: rule_name.as_ref().map(|_| Uuid::new_v4().to_string()),
            rule_name,
            explanation: None,
            undo_status: UndoStatus::Completed,
        };
        super::upsert_audit_entry(&fixture.db, &entry).expect("audit entry should save");
        entry
    }

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

        super::upsert_audit_entry(&fixture.db, &entry).expect("audit entry should save");

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

    #[test]
    fn audit_pages_use_a_stable_sequence_cursor() {
        let fixture = Fixture::new("shelflife-audit-pagination");
        for index in 0..55 {
            save_audit_entry(
                &fixture,
                format!("entry-{index}.txt").as_str(),
                None,
                None,
                AuditActionKind::Pin,
                None,
            );
        }

        let first = super::list_audit_entries_page(&fixture.db, None, "")
            .expect("first audit page should load");
        assert_eq!(first.entries.len(), 50);
        assert_eq!(first.total_count, Some(55));
        assert!(first.has_more);
        assert_eq!(first.entries.first().map(|entry| entry.sequence), Some(55));
        assert_eq!(first.entries.last().map(|entry| entry.sequence), Some(6));

        let second = super::list_audit_entries_page(&fixture.db, Some(6), "")
            .expect("second audit page should load");
        assert_eq!(second.entries.len(), 5);
        assert_eq!(second.total_count, None);
        assert!(!second.has_more);
        assert_eq!(second.entries.first().map(|entry| entry.sequence), Some(5));
        assert_eq!(second.entries.last().map(|entry| entry.sequence), Some(1));
    }

    #[test]
    fn audit_page_search_matches_the_existing_fields_and_literal_wildcards() {
        let fixture = Fixture::new("shelflife-audit-search");
        let file_name = save_audit_entry(
            &fixture,
            "file-name-hit.txt",
            None,
            None,
            AuditActionKind::Pin,
            None,
        );
        let source = save_audit_entry(
            &fixture,
            "source.txt",
            Some(String::from("C:\\source-path-hit\\source.txt")),
            None,
            AuditActionKind::Pin,
            None,
        );
        let destination = save_audit_entry(
            &fixture,
            "destination.txt",
            None,
            Some(String::from("C:\\destination-path-hit\\destination.txt")),
            AuditActionKind::Pin,
            None,
        );
        let rule = save_audit_entry(
            &fixture,
            "rule.txt",
            None,
            None,
            AuditActionKind::Pin,
            Some(String::from("rule-name-hit")),
        );
        let action = save_audit_entry(
            &fixture,
            "action.txt",
            None,
            None,
            AuditActionKind::Move,
            None,
        );
        let percent = save_audit_entry(
            &fixture,
            "literal%-hit.txt",
            None,
            None,
            AuditActionKind::Pin,
            None,
        );

        for (query, expected_id) in [
            ("FILE-NAME-HIT", file_name.id.as_str()),
            ("source-path-hit", source.id.as_str()),
            ("destination-path-hit", destination.id.as_str()),
            ("rule-name-hit", rule.id.as_str()),
            ("MOVE", action.id.as_str()),
            ("%", percent.id.as_str()),
        ] {
            let page = super::list_audit_entries_page(&fixture.db, None, query)
                .expect("searched audit page should load");
            assert_eq!(page.total_count, Some(1), "query={query}");
            assert_eq!(
                page.entries.first().map(|entry| entry.id.as_str()),
                Some(expected_id),
                "query={query}"
            );
            assert!(!page.has_more, "query={query}");
        }
    }
}
