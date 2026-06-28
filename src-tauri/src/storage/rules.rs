use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::models::{AppError, AutomationRule, RuleConditions, SizeCondition};
use crate::storage::{
    i64_to_u64, insert_ordered_values, load_ordered_values, load_ordered_values_by_owner,
    rule_action_from_parts, rule_action_parts, rule_mode_from_label, rule_mode_label,
    storage_data_error, u64_to_i64, Database,
};

pub fn list_rules(db: &Database) -> Result<Vec<AutomationRule>, AppError> {
    let conn = db.connect()?;
    let mut stmt = conn.prepare(
        "
        SELECT id,
               name,
               enabled,
               priority,
               watch_path,
               ttl_seconds,
               mode,
               created_at,
               updated_at,
               action_kind,
               action_destination_folder,
               action_rename_template,
               size_kind,
               size_min,
               size_max
        FROM automation_rules
        ORDER BY priority DESC, name COLLATE NOCASE ASC
        ",
    )?;
    let rows = stmt.query_map([], rule_row_from_sql)?;
    let mut rule_rows = Vec::new();
    for row in rows {
        rule_rows.push(row?);
    }

    let mut extensions_by_rule =
        load_ordered_values_by_owner(&conn, "rule_extensions", "rule_id", "value")?;
    let mut globs_by_rule =
        load_ordered_values_by_owner(&conn, "rule_filename_globs", "rule_id", "value")?;
    let mut regexes_by_rule =
        load_ordered_values_by_owner(&conn, "rule_filename_regexes", "rule_id", "value")?;
    let mut domains_by_rule =
        load_ordered_values_by_owner(&conn, "rule_source_domains", "rule_id", "value")?;

    let mut rules = Vec::new();
    for row in rule_rows {
        let rule_id = row.id.clone();
        let children = RuleChildValues {
            extensions: extensions_by_rule.remove(&rule_id).unwrap_or_default(),
            filename_globs: globs_by_rule.remove(&rule_id).unwrap_or_default(),
            filename_regexes: regexes_by_rule.remove(&rule_id).unwrap_or_default(),
            source_domains: domains_by_rule.remove(&rule_id).unwrap_or_default(),
        };
        rules.push(rule_from_row(row, children)?);
    }
    Ok(rules)
}

pub fn get_rule(db: &Database, id: &str) -> Result<Option<AutomationRule>, AppError> {
    let conn = db.connect()?;
    let row = conn
        .query_row(
            "
            SELECT id,
                   name,
                   enabled,
                   priority,
                   watch_path,
                   ttl_seconds,
                   mode,
                   created_at,
                   updated_at,
                   action_kind,
                   action_destination_folder,
                   action_rename_template,
                   size_kind,
                   size_min,
                   size_max
            FROM automation_rules
            WHERE id = ?1
            ",
            params![id],
            rule_row_from_sql,
        )
        .optional()?;

    row.map(|row| {
        let children = load_rule_child_values(&conn, &row.id)?;
        rule_from_row(row, children)
    })
    .transpose()
}

pub fn save_rule(db: &Database, rule: &AutomationRule) -> Result<(), AppError> {
    db.write(|tx| {
        save_rule_tx(tx, rule)?;
        Ok(())
    })
}

fn save_rule_tx(tx: &Transaction<'_>, rule: &AutomationRule) -> Result<(), AppError> {
    let (action_kind, action_destination_folder, action_rename_template) =
        rule_action_parts(&rule.action);
    let (size_kind, size_min, size_max) = size_condition_parts(&rule.conditions.size)?;

    tx.execute(
        "
        INSERT INTO automation_rules (
            id,
            name,
            enabled,
            priority,
            watch_path,
            ttl_seconds,
            mode,
            created_at,
            updated_at,
            action_kind,
            action_destination_folder,
            action_rename_template,
            size_kind,
            size_min,
            size_max
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            enabled = excluded.enabled,
            priority = excluded.priority,
            watch_path = excluded.watch_path,
            ttl_seconds = excluded.ttl_seconds,
            mode = excluded.mode,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at,
            action_kind = excluded.action_kind,
            action_destination_folder = excluded.action_destination_folder,
            action_rename_template = excluded.action_rename_template,
            size_kind = excluded.size_kind,
            size_min = excluded.size_min,
            size_max = excluded.size_max
        ",
        params![
            &rule.id,
            &rule.name,
            rule.enabled,
            rule.priority,
            &rule.watch_path,
            u64_to_i64(rule.ttl_seconds, "automation_rules.ttl_seconds")?,
            rule_mode_label(&rule.mode),
            u64_to_i64(rule.created_at, "automation_rules.created_at")?,
            u64_to_i64(rule.updated_at, "automation_rules.updated_at")?,
            action_kind,
            action_destination_folder,
            action_rename_template,
            size_kind,
            size_min,
            size_max,
        ],
    )?;

    // The parent row survives on upsert, so cascades do not clear replaced condition lists.
    delete_rule_children(tx, &rule.id)?;
    insert_ordered_values(
        tx,
        "rule_extensions",
        "rule_id",
        &rule.id,
        &rule.conditions.extensions,
    )?;
    insert_ordered_values(
        tx,
        "rule_filename_globs",
        "rule_id",
        &rule.id,
        &rule.conditions.filename_globs,
    )?;
    insert_ordered_values(
        tx,
        "rule_filename_regexes",
        "rule_id",
        &rule.id,
        &rule.conditions.filename_regexes,
    )?;
    insert_ordered_values(
        tx,
        "rule_source_domains",
        "rule_id",
        &rule.id,
        &rule.conditions.source_domains,
    )?;
    Ok(())
}

pub fn delete_rule(db: &Database, id: &str) -> Result<(), AppError> {
    db.write(|tx| {
        tx.execute("DELETE FROM automation_rules WHERE id = ?1", params![id])?;
        Ok(())
    })
}

fn delete_rule_children(tx: &Transaction<'_>, rule_id: &str) -> Result<(), AppError> {
    tx.execute(
        "DELETE FROM rule_extensions WHERE rule_id = ?1",
        params![rule_id],
    )?;
    tx.execute(
        "DELETE FROM rule_filename_globs WHERE rule_id = ?1",
        params![rule_id],
    )?;
    tx.execute(
        "DELETE FROM rule_filename_regexes WHERE rule_id = ?1",
        params![rule_id],
    )?;
    tx.execute(
        "DELETE FROM rule_source_domains WHERE rule_id = ?1",
        params![rule_id],
    )?;
    Ok(())
}

fn load_rule_child_values(conn: &Connection, rule_id: &str) -> Result<RuleChildValues, AppError> {
    Ok(RuleChildValues {
        extensions: load_ordered_values(conn, "rule_extensions", "rule_id", rule_id)?,
        filename_globs: load_ordered_values(conn, "rule_filename_globs", "rule_id", rule_id)?,
        filename_regexes: load_ordered_values(conn, "rule_filename_regexes", "rule_id", rule_id)?,
        source_domains: load_ordered_values(conn, "rule_source_domains", "rule_id", rule_id)?,
    })
}

fn rule_from_row(row: RuleRow, children: RuleChildValues) -> Result<AutomationRule, AppError> {
    let action = rule_action_from_parts(
        &row.action_kind,
        row.action_destination_folder,
        row.action_rename_template,
    )?;
    let conditions = RuleConditions {
        extensions: children.extensions,
        filename_globs: children.filename_globs,
        filename_regexes: children.filename_regexes,
        source_domains: children.source_domains,
        size: size_condition_from_parts(&row.size_kind, row.size_min, row.size_max)?,
    };

    Ok(AutomationRule {
        id: row.id,
        name: row.name,
        enabled: row.enabled,
        priority: row.priority,
        watch_path: row.watch_path,
        ttl_seconds: i64_to_u64(row.ttl_seconds, "automation_rules.ttl_seconds")?,
        conditions,
        action,
        mode: rule_mode_from_label(&row.mode)?,
        created_at: i64_to_u64(row.created_at, "automation_rules.created_at")?,
        updated_at: i64_to_u64(row.updated_at, "automation_rules.updated_at")?,
    })
}

fn rule_row_from_sql(row: &Row<'_>) -> rusqlite::Result<RuleRow> {
    Ok(RuleRow {
        id: row.get(0)?,
        name: row.get(1)?,
        enabled: row.get(2)?,
        priority: row.get(3)?,
        watch_path: row.get(4)?,
        ttl_seconds: row.get(5)?,
        mode: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        action_kind: row.get(9)?,
        action_destination_folder: row.get(10)?,
        action_rename_template: row.get(11)?,
        size_kind: row.get(12)?,
        size_min: row.get(13)?,
        size_max: row.get(14)?,
    })
}

struct RuleRow {
    id: String,
    name: String,
    enabled: bool,
    priority: i32,
    watch_path: String,
    ttl_seconds: i64,
    mode: String,
    created_at: i64,
    updated_at: i64,
    action_kind: String,
    action_destination_folder: Option<String>,
    action_rename_template: Option<String>,
    size_kind: String,
    size_min: Option<i64>,
    size_max: Option<i64>,
}

struct RuleChildValues {
    extensions: Vec<String>,
    filename_globs: Vec<String>,
    filename_regexes: Vec<String>,
    source_domains: Vec<String>,
}

fn size_condition_parts(
    condition: &SizeCondition,
) -> Result<(&'static str, Option<i64>, Option<i64>), AppError> {
    match condition {
        SizeCondition::Any => Ok(("any", None, None)),
        SizeCondition::LessThan(value) => Ok((
            "less_than",
            None,
            Some(u64_to_i64(*value, "automation_rules.size_max")?),
        )),
        SizeCondition::GreaterThan(value) => Ok((
            "greater_than",
            Some(u64_to_i64(*value, "automation_rules.size_min")?),
            None,
        )),
        SizeCondition::Between { min, max } => Ok((
            "between",
            Some(u64_to_i64(*min, "automation_rules.size_min")?),
            Some(u64_to_i64(*max, "automation_rules.size_max")?),
        )),
    }
}

fn size_condition_from_parts(
    kind: &str,
    min: Option<i64>,
    max: Option<i64>,
) -> Result<SizeCondition, AppError> {
    match kind {
        "any" => Ok(SizeCondition::Any),
        "less_than" => Ok(SizeCondition::LessThan(i64_to_u64(
            max.ok_or_else(|| {
                storage_data_error(
                    "Stored less-than size condition is missing its maximum.",
                    kind,
                )
            })?,
            "automation_rules.size_max",
        )?)),
        "greater_than" => Ok(SizeCondition::GreaterThan(i64_to_u64(
            min.ok_or_else(|| {
                storage_data_error(
                    "Stored greater-than size condition is missing its minimum.",
                    kind,
                )
            })?,
            "automation_rules.size_min",
        )?)),
        "between" => Ok(SizeCondition::Between {
            min: i64_to_u64(
                min.ok_or_else(|| {
                    storage_data_error("Stored size condition is missing its minimum.", kind)
                })?,
                "automation_rules.size_min",
            )?,
            max: i64_to_u64(
                max.ok_or_else(|| {
                    storage_data_error("Stored size condition is missing its maximum.", kind)
                })?,
                "automation_rules.size_max",
            )?,
        }),
        other => Err(storage_data_error(
            "Stored size condition is not recognized.",
            other,
        )),
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{RuleAction, RuleMode, SizeCondition};
    use crate::storage::test_util::{path_string, Fixture};

    #[test]
    fn rule_round_trips_all_normalized_condition_and_action_fields() {
        let fixture = Fixture::new("shelflife-rule-round-trip");
        let mut rule = fixture.rule();
        rule.mode = RuleMode::Automatic;
        rule.action = RuleAction::Move {
            destination_folder: path_string(&fixture.safe),
            rename_template: Some(String::from("{name}-archived.{ext}")),
        };
        rule.conditions.extensions = vec![String::from("zip"), String::from("tar.gz")];
        rule.conditions.filename_globs = vec![String::from("*backup*")];
        rule.conditions.filename_regexes = vec![String::from("(?i)download")];
        rule.conditions.source_domains = vec![String::from("example.com")];
        rule.conditions.size = SizeCondition::Between { min: 10, max: 20 };

        super::save_rule(&fixture.db, &rule).expect("rule should save");

        let loaded = super::get_rule(&fixture.db, &rule.id)
            .expect("rule lookup should work")
            .expect("rule should exist");
        assert_eq!(loaded, rule);
    }
}
