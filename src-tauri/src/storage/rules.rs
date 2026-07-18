use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::Text;
use diesel::sqlite::SqliteConnection;

use crate::models::{AppError, AutomationRule, RuleConditions, SizeCondition};
use crate::storage::schema::automation_rules;
use crate::storage::{
    delete_owner_rows, i64_to_u64, insert_ordered_values, load_ordered_values,
    load_ordered_values_by_owner, rule_action_from_parts, rule_action_parts, rule_mode_from_label,
    rule_mode_label, rule_timing_from_parts, rule_timing_parts, storage_data_error, u64_to_i64,
    Database,
};

pub fn list_rules(db: &Database) -> Result<Vec<AutomationRule>, AppError> {
    let mut conn = db.connect()?;
    let rule_rows = automation_rules::table
        .order((
            automation_rules::priority.desc(),
            sql::<Text>("name COLLATE NOCASE ASC"),
        ))
        .select(RuleRow::as_select())
        .load::<RuleRow>(&mut conn)?;

    let mut extensions_by_rule =
        load_ordered_values_by_owner(&mut conn, "rule_extensions", "rule_id", "value")?;
    let mut globs_by_rule =
        load_ordered_values_by_owner(&mut conn, "rule_filename_globs", "rule_id", "value")?;
    let mut regexes_by_rule =
        load_ordered_values_by_owner(&mut conn, "rule_filename_regexes", "rule_id", "value")?;
    let mut domains_by_rule =
        load_ordered_values_by_owner(&mut conn, "rule_source_domains", "rule_id", "value")?;

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
    let mut conn = db.connect()?;
    let row = automation_rules::table
        .find(id)
        .select(RuleRow::as_select())
        .first::<RuleRow>(&mut conn)
        .optional()?;

    row.map(|row| {
        let children = load_rule_child_values(&mut conn, &row.id)?;
        rule_from_row(row, children)
    })
    .transpose()
}

pub fn save_rule(db: &Database, rule: &AutomationRule) -> Result<(), AppError> {
    db.write(|conn| {
        save_rule_tx(conn, rule)?;
        Ok(())
    })
}

fn save_rule_tx(conn: &mut SqliteConnection, rule: &AutomationRule) -> Result<(), AppError> {
    let (action_kind, action_destination_folder, action_rename_template) =
        rule_action_parts(&rule.action);
    let (timing_kind, timing_seconds) = rule_timing_parts(&rule.timing);
    let (size_kind, size_min, size_max) = size_condition_parts(&rule.conditions.size)?;
    let row = RuleWriteRow {
        id: &rule.id,
        name: &rule.name,
        enabled: rule.enabled,
        priority: rule.priority,
        watch_path: &rule.watch_path,
        ttl_seconds: u64_to_i64(timing_seconds, "automation_rules.ttl_seconds")?,
        timing_kind,
        mode: rule_mode_label(&rule.mode),
        created_at: u64_to_i64(rule.created_at, "automation_rules.created_at")?,
        updated_at: u64_to_i64(rule.updated_at, "automation_rules.updated_at")?,
        action_kind,
        action_destination_folder,
        action_rename_template,
        size_kind,
        size_min,
        size_max,
    };

    diesel::insert_into(automation_rules::table)
        .values(&row)
        .on_conflict(automation_rules::id)
        .do_update()
        .set(&row)
        .execute(conn)?;

    // The parent row survives on upsert, so cascades do not clear replaced condition lists.
    delete_rule_children(conn, &rule.id)?;
    insert_ordered_values(
        conn,
        "rule_extensions",
        "rule_id",
        "value",
        &rule.id,
        &rule.conditions.extensions,
        "rule_extensions.ordinal",
    )?;
    insert_ordered_values(
        conn,
        "rule_filename_globs",
        "rule_id",
        "value",
        &rule.id,
        &rule.conditions.filename_globs,
        "rule_filename_globs.ordinal",
    )?;
    insert_ordered_values(
        conn,
        "rule_filename_regexes",
        "rule_id",
        "value",
        &rule.id,
        &rule.conditions.filename_regexes,
        "rule_filename_regexes.ordinal",
    )?;
    insert_ordered_values(
        conn,
        "rule_source_domains",
        "rule_id",
        "value",
        &rule.id,
        &rule.conditions.source_domains,
        "rule_source_domains.ordinal",
    )?;
    Ok(())
}

pub fn delete_rule(db: &Database, id: &str) -> Result<(), AppError> {
    db.write(|conn| {
        diesel::delete(automation_rules::table.filter(automation_rules::id.eq(id)))
            .execute(conn)?;
        Ok(())
    })
}

fn delete_rule_children(conn: &mut SqliteConnection, rule_id: &str) -> Result<(), AppError> {
    delete_owner_rows(conn, "rule_extensions", "rule_id", rule_id)?;
    delete_owner_rows(conn, "rule_filename_globs", "rule_id", rule_id)?;
    delete_owner_rows(conn, "rule_filename_regexes", "rule_id", rule_id)?;
    delete_owner_rows(conn, "rule_source_domains", "rule_id", rule_id)?;
    Ok(())
}

fn load_rule_child_values(
    conn: &mut SqliteConnection,
    rule_id: &str,
) -> Result<RuleChildValues, AppError> {
    Ok(RuleChildValues {
        extensions: load_ordered_values(conn, "rule_extensions", "rule_id", rule_id, "value")?,
        filename_globs: load_ordered_values(
            conn,
            "rule_filename_globs",
            "rule_id",
            rule_id,
            "value",
        )?,
        filename_regexes: load_ordered_values(
            conn,
            "rule_filename_regexes",
            "rule_id",
            rule_id,
            "value",
        )?,
        source_domains: load_ordered_values(
            conn,
            "rule_source_domains",
            "rule_id",
            rule_id,
            "value",
        )?,
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
        timing: rule_timing_from_parts(&row.timing_kind, row.ttl_seconds)?,
        conditions,
        action,
        mode: rule_mode_from_label(&row.mode)?,
        created_at: i64_to_u64(row.created_at, "automation_rules.created_at")?,
        updated_at: i64_to_u64(row.updated_at, "automation_rules.updated_at")?,
    })
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = automation_rules)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct RuleRow {
    id: String,
    name: String,
    enabled: bool,
    priority: i32,
    watch_path: String,
    ttl_seconds: i64,
    timing_kind: String,
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

#[derive(Insertable, AsChangeset)]
#[diesel(table_name = automation_rules)]
#[diesel(treat_none_as_null = true)]
struct RuleWriteRow<'a> {
    id: &'a str,
    name: &'a str,
    enabled: bool,
    priority: i32,
    watch_path: &'a str,
    ttl_seconds: i64,
    timing_kind: &'a str,
    mode: &'a str,
    created_at: i64,
    updated_at: i64,
    action_kind: &'a str,
    action_destination_folder: Option<&'a str>,
    action_rename_template: Option<&'a str>,
    size_kind: &'a str,
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
    use crate::models::{RuleAction, RuleMode, RuleTiming, SizeCondition};
    use crate::storage::test_util::{path_string, Fixture};

    #[test]
    fn rule_round_trips_all_normalized_condition_and_action_fields() {
        let fixture = Fixture::new("shelflife-rule-round-trip");
        let mut rule = fixture.rule();
        rule.mode = RuleMode::Automatic;
        rule.timing = RuleTiming::OnArrival;
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
