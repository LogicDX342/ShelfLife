pub mod audit;
pub mod rules;
pub(crate) mod schema;
#[cfg(test)]
pub mod test_util;
pub mod tracked;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Integer, Text};
use diesel::sqlite::SqliteConnection;

use crate::models::{
    AppConfig, AppError, AuditEntry, CloseBehavior, RuleAction, RuleMode, TrackedFile, WatchTarget,
};
use crate::storage::schema::{app_config, watch_targets};

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone)]
pub struct Database {
    path: PathBuf,
}

impl Database {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub(crate) fn connect(&self) -> Result<SqliteConnection, AppError> {
        let database_url = self.path.to_string_lossy().into_owned();
        let mut conn = SqliteConnection::establish(&database_url)?;
        configure_connection(&mut conn)?;
        Ok(conn)
    }

    pub(crate) fn write<T>(
        &self,
        write: impl FnOnce(&mut SqliteConnection) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let mut conn = self.connect()?;
        conn.immediate_transaction(|conn| write(conn))
    }
}

pub fn open_database(path: impl AsRef<Path>) -> Result<Database, AppError> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db = Database::new(path.as_ref().to_path_buf());
    initialize_database(&db)?;
    Ok(db)
}

pub(crate) fn finalize_file_action(
    db: &Database,
    original_path: &str,
    file: &TrackedFile,
    audit_entry: &AuditEntry,
) -> Result<(), AppError> {
    db.write(|conn| {
        tracked::replace_tracked_file_tx(conn, original_path, file)?;
        audit::upsert_audit_entry_tx(conn, audit_entry)
    })
}

fn configure_connection(conn: &mut SqliteConnection) -> Result<(), AppError> {
    sql_query("PRAGMA busy_timeout = 5000").execute(conn)?;
    sql_query("PRAGMA foreign_keys = ON").execute(conn)?;
    sql_query("PRAGMA journal_mode = WAL").execute(conn)?;
    sql_query("PRAGMA synchronous = NORMAL").execute(conn)?;
    Ok(())
}

fn initialize_database(db: &Database) -> Result<(), AppError> {
    let mut conn = db.connect()?;
    let version_row = sql_query("PRAGMA user_version").get_result::<UserVersionRow>(&mut conn)?;
    let version = i64::from(version_row.user_version);
    if version > SCHEMA_VERSION {
        return Err(AppError::with_details(
            "DATABASE_ERROR",
            "Database schema is newer than this ShelfLife build supports.",
            true,
            format!("database={version}, supported={SCHEMA_VERSION}"),
        ));
    }

    for statement in SCHEMA_SQL
        .split(';')
        .map(str::trim)
        .filter(|sql| !sql.is_empty())
    {
        sql_query(statement).execute(&mut conn)?;
    }
    sql_query(format!("PRAGMA user_version = {SCHEMA_VERSION}")).execute(&mut conn)?;
    Ok(())
}

#[derive(QueryableByName)]
struct UserVersionRow {
    #[diesel(sql_type = Integer)]
    user_version: i32,
}

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS app_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    default_ttl_seconds INTEGER NOT NULL,
    stale_threshold_seconds INTEGER NOT NULL,
    decaying_threshold_seconds INTEGER NOT NULL,
    safe_folder_path TEXT NOT NULL,
    notifications_enabled INTEGER NOT NULL CHECK (notifications_enabled IN (0, 1)),
    start_at_login INTEGER NOT NULL CHECK (start_at_login IN (0, 1)),
    close_behavior TEXT NOT NULL CHECK (close_behavior IN ('ask', 'hide_to_tray', 'quit')),
    dropzone_enabled INTEGER NOT NULL CHECK (dropzone_enabled IN (0, 1))
);

CREATE TABLE IF NOT EXISTS watch_targets (
    id TEXT PRIMARY KEY,
    ordinal INTEGER NOT NULL,
    path TEXT NOT NULL,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    recursive INTEGER NOT NULL CHECK (recursive IN (0, 1))
);

CREATE TABLE IF NOT EXISTS watch_target_ignore_patterns (
    target_id TEXT NOT NULL REFERENCES watch_targets(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (target_id, ordinal)
);

CREATE TABLE IF NOT EXISTS watch_target_include_hidden_patterns (
    target_id TEXT NOT NULL REFERENCES watch_targets(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (target_id, ordinal)
);

CREATE TABLE IF NOT EXISTS automation_rules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    priority INTEGER NOT NULL,
    watch_path TEXT NOT NULL,
    ttl_seconds INTEGER NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('preview_only', 'ask_first', 'automatic')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    action_kind TEXT NOT NULL CHECK (action_kind IN ('trash', 'move', 'ignore')),
    action_destination_folder TEXT,
    action_rename_template TEXT,
    size_kind TEXT NOT NULL CHECK (size_kind IN ('any', 'less_than', 'greater_than', 'between')),
    size_min INTEGER,
    size_max INTEGER,
    CHECK (
        (size_kind = 'any' AND size_min IS NULL AND size_max IS NULL)
        OR (size_kind = 'less_than' AND size_min IS NULL AND size_max IS NOT NULL)
        OR (size_kind = 'greater_than' AND size_min IS NOT NULL AND size_max IS NULL)
        OR (size_kind = 'between' AND size_min IS NOT NULL AND size_max IS NOT NULL AND size_min <= size_max)
    )
);

CREATE TABLE IF NOT EXISTS rule_extensions (
    rule_id TEXT NOT NULL REFERENCES automation_rules(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (rule_id, ordinal)
);

CREATE TABLE IF NOT EXISTS rule_filename_globs (
    rule_id TEXT NOT NULL REFERENCES automation_rules(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (rule_id, ordinal)
);

CREATE TABLE IF NOT EXISTS rule_filename_regexes (
    rule_id TEXT NOT NULL REFERENCES automation_rules(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (rule_id, ordinal)
);

CREATE TABLE IF NOT EXISTS rule_source_domains (
    rule_id TEXT NOT NULL REFERENCES automation_rules(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (rule_id, ordinal)
);

CREATE TABLE IF NOT EXISTS tracked_files (
    path TEXT PRIMARY KEY,
    file_name TEXT NOT NULL,
    watch_target_id TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    first_seen_at INTEGER NOT NULL,
    last_observed_mtime INTEGER,
    last_observed_atime INTEGER,
    last_user_action_at INTEGER,
    freshness_at INTEGER NOT NULL,
    expiry_kind TEXT NOT NULL CHECK (expiry_kind IN ('at', 'permanent', 'snoozed_until')),
    expires_at INTEGER,
    state TEXT NOT NULL CHECK (state IN ('fresh', 'stale', 'decaying', 'pinned', 'ignored', 'missing')),
    origin_kind TEXT NOT NULL CHECK (origin_kind IN ('mac_where_froms', 'windows_zone_identifier', 'linux_xattr', 'unknown')),
    origin_zone_id INTEGER,
    origin_host_url TEXT,
    origin_referrer_url TEXT,
    origin_xattr_key TEXT,
    origin_xattr_value_utf8 TEXT
);

CREATE TABLE IF NOT EXISTS tracked_file_rules (
    file_path TEXT NOT NULL REFERENCES tracked_files(path) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    rule_id TEXT NOT NULL,
    PRIMARY KEY (file_path, ordinal),
    UNIQUE (file_path, rule_id)
);

CREATE TABLE IF NOT EXISTS origin_values (
    file_path TEXT NOT NULL REFERENCES tracked_files(path) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (file_path, ordinal)
);

CREATE TABLE IF NOT EXISTS audit_sequence_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    next_sequence INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_entries (
    sequence INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    timestamp INTEGER NOT NULL,
    action_kind TEXT NOT NULL CHECK (action_kind IN ('trash', 'move', 'pin', 'snooze', 'ignore')),
    source_path TEXT NOT NULL,
    destination_path TEXT,
    file_name TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    rule_id TEXT,
    rule_name TEXT,
    undo_status_kind TEXT NOT NULL CHECK (undo_status_kind IN ('available', 'unavailable', 'completed', 'failed')),
    undo_status_reason TEXT,
    explanation_file_path TEXT,
    explanation_size_bytes INTEGER,
    explanation_rule_id TEXT,
    explanation_rule_name TEXT,
    explanation_matched_extension INTEGER CHECK (explanation_matched_extension IN (0, 1)),
    explanation_matched_size INTEGER CHECK (explanation_matched_size IN (0, 1)),
    explanation_matched_origin TEXT,
    explanation_matched_filename_pattern TEXT,
    explanation_proposed_action_kind TEXT CHECK (explanation_proposed_action_kind IN ('trash', 'move', 'ignore')),
    explanation_proposed_action_destination_folder TEXT,
    explanation_proposed_action_rename_template TEXT,
    explanation_mode TEXT CHECK (explanation_mode IN ('preview_only', 'ask_first', 'automatic')),
    explanation_message TEXT
);

CREATE INDEX IF NOT EXISTS idx_watch_targets_ordinal ON watch_targets(ordinal);
CREATE INDEX IF NOT EXISTS idx_rules_order ON automation_rules(priority DESC, name COLLATE NOCASE ASC);
CREATE INDEX IF NOT EXISTS idx_tracked_state ON tracked_files(state);
CREATE INDEX IF NOT EXISTS idx_tracked_expiry ON tracked_files(expires_at);
CREATE INDEX IF NOT EXISTS idx_tracked_file_rules_rule_id ON tracked_file_rules(rule_id);
CREATE INDEX IF NOT EXISTS idx_audit_failed_rules
    ON audit_entries(undo_status_kind, source_path, rule_id)
    WHERE rule_id IS NOT NULL;
"#;

pub fn get_config(db: &Database) -> Result<AppConfig, AppError> {
    let mut conn = db.connect()?;
    let Some(config) = app_config::table
        .find(1_i32)
        .select(ConfigRow::as_select())
        .first(&mut conn)
        .optional()?
    else {
        return Ok(AppConfig::default());
    };

    let target_rows = watch_targets::table
        .order(watch_targets::ordinal.asc())
        .select(WatchTargetRow::as_select())
        .load::<WatchTargetRow>(&mut conn)?;

    let mut watch_targets = Vec::new();
    for row in target_rows {
        watch_targets.push(WatchTarget {
            ignore_patterns: load_ordered_values(
                &mut conn,
                "watch_target_ignore_patterns",
                "target_id",
                &row.id,
                "value",
            )?,
            include_hidden_patterns: load_ordered_values(
                &mut conn,
                "watch_target_include_hidden_patterns",
                "target_id",
                &row.id,
                "value",
            )?,
            id: row.id,
            path: row.path,
            enabled: row.enabled,
            recursive: row.recursive,
        });
    }

    Ok(AppConfig {
        watch_targets,
        default_ttl_seconds: i64_to_u64(config.default_ttl_seconds, "default_ttl_seconds")?,
        stale_threshold_seconds: i64_to_u64(
            config.stale_threshold_seconds,
            "stale_threshold_seconds",
        )?,
        decaying_threshold_seconds: i64_to_u64(
            config.decaying_threshold_seconds,
            "decaying_threshold_seconds",
        )?,
        safe_folder_path: config.safe_folder_path,
        notifications_enabled: config.notifications_enabled,
        start_at_login: config.start_at_login,
        close_behavior: close_behavior_from_label(&config.close_behavior)?,
        dropzone_enabled: config.dropzone_enabled,
    })
}

pub fn save_config(db: &Database, config: &AppConfig) -> Result<(), AppError> {
    db.write(|conn| {
        let row = ConfigWriteRow {
            id: 1,
            default_ttl_seconds: u64_to_i64(config.default_ttl_seconds, "default_ttl_seconds")?,
            stale_threshold_seconds: u64_to_i64(
                config.stale_threshold_seconds,
                "stale_threshold_seconds",
            )?,
            decaying_threshold_seconds: u64_to_i64(
                config.decaying_threshold_seconds,
                "decaying_threshold_seconds",
            )?,
            safe_folder_path: &config.safe_folder_path,
            notifications_enabled: config.notifications_enabled,
            start_at_login: config.start_at_login,
            close_behavior: close_behavior_label(&config.close_behavior),
            dropzone_enabled: config.dropzone_enabled,
        };

        diesel::insert_into(app_config::table)
            .values(&row)
            .on_conflict(app_config::id)
            .do_update()
            .set(&row)
            .execute(conn)?;

        diesel::delete(watch_targets::table).execute(conn)?;
        for (ordinal, target) in config.watch_targets.iter().enumerate() {
            let target_row = WatchTargetWriteRow {
                id: &target.id,
                ordinal: usize_to_i64(ordinal, "watch_targets.ordinal")?,
                path: &target.path,
                enabled: target.enabled,
                recursive: target.recursive,
            };
            diesel::insert_into(watch_targets::table)
                .values(&target_row)
                .execute(conn)?;
            insert_ordered_values(
                conn,
                "watch_target_ignore_patterns",
                "target_id",
                "value",
                &target.id,
                &target.ignore_patterns,
                "watch_target_ignore_patterns.ordinal",
            )?;
            insert_ordered_values(
                conn,
                "watch_target_include_hidden_patterns",
                "target_id",
                "value",
                &target.id,
                &target.include_hidden_patterns,
                "watch_target_include_hidden_patterns.ordinal",
            )?;
        }

        Ok(())
    })
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = app_config)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct ConfigRow {
    default_ttl_seconds: i64,
    stale_threshold_seconds: i64,
    decaying_threshold_seconds: i64,
    safe_folder_path: String,
    notifications_enabled: bool,
    start_at_login: bool,
    close_behavior: String,
    dropzone_enabled: bool,
}

#[derive(Insertable, AsChangeset)]
#[diesel(table_name = app_config)]
struct ConfigWriteRow<'a> {
    id: i32,
    default_ttl_seconds: i64,
    stale_threshold_seconds: i64,
    decaying_threshold_seconds: i64,
    safe_folder_path: &'a str,
    notifications_enabled: bool,
    start_at_login: bool,
    close_behavior: &'a str,
    dropzone_enabled: bool,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = watch_targets)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct WatchTargetRow {
    id: String,
    path: String,
    enabled: bool,
    recursive: bool,
}

#[derive(Insertable)]
#[diesel(table_name = watch_targets)]
struct WatchTargetWriteRow<'a> {
    id: &'a str,
    ordinal: i64,
    path: &'a str,
    enabled: bool,
    recursive: bool,
}

#[derive(QueryableByName)]
struct ValueRow {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct OwnerValueRow {
    #[diesel(sql_type = Text)]
    owner_id: String,
    #[diesel(sql_type = Text)]
    value: String,
}

pub(crate) fn load_ordered_values(
    conn: &mut SqliteConnection,
    table: &str,
    owner_column: &str,
    owner_id: &str,
    value_column: &str,
) -> Result<Vec<String>, AppError> {
    let sql = format!(
        "SELECT {value_column} AS value FROM {table} WHERE {owner_column} = ? ORDER BY ordinal ASC"
    );
    let rows = sql_query(sql.as_str())
        .bind::<Text, _>(owner_id)
        .load::<ValueRow>(conn)?;
    Ok(rows.into_iter().map(|row| row.value).collect())
}

pub(crate) fn load_ordered_values_by_owner(
    conn: &mut SqliteConnection,
    table: &str,
    owner_column: &str,
    value_column: &str,
) -> Result<HashMap<String, Vec<String>>, AppError> {
    let sql = format!(
        "SELECT {owner_column} AS owner_id, {value_column} AS value FROM {table} ORDER BY {owner_column} ASC, ordinal ASC"
    );
    let rows = sql_query(sql.as_str()).load::<OwnerValueRow>(conn)?;
    Ok(collect_ordered_values_by_owner(
        rows.into_iter()
            .map(|row| (row.owner_id, row.value))
            .collect(),
    ))
}

pub(crate) fn delete_owner_rows(
    conn: &mut SqliteConnection,
    table: &str,
    owner_column: &str,
    owner_id: &str,
) -> Result<(), AppError> {
    let sql = format!("DELETE FROM {table} WHERE {owner_column} = ?");
    sql_query(sql.as_str())
        .bind::<Text, _>(owner_id)
        .execute(conn)?;
    Ok(())
}

pub(crate) fn insert_ordered_values(
    conn: &mut SqliteConnection,
    table: &str,
    owner_column: &str,
    value_column: &str,
    owner_id: &str,
    values: &[String],
    ordinal_field: &str,
) -> Result<(), AppError> {
    if values.is_empty() {
        return Ok(());
    }

    let sql =
        format!("INSERT INTO {table} ({owner_column}, ordinal, {value_column}) VALUES (?, ?, ?)");
    for (ordinal, value) in values.iter().enumerate() {
        sql_query(sql.as_str())
            .bind::<Text, _>(owner_id)
            .bind::<BigInt, _>(usize_to_i64(ordinal, ordinal_field)?)
            .bind::<Text, _>(value)
            .execute(conn)?;
    }
    Ok(())
}

pub(crate) fn collect_ordered_values_by_owner(
    rows: Vec<(String, String)>,
) -> HashMap<String, Vec<String>> {
    let mut values_by_owner: HashMap<String, Vec<String>> = HashMap::new();
    for (owner_id, value) in rows {
        values_by_owner.entry(owner_id).or_default().push(value);
    }
    values_by_owner
}

pub(crate) fn u64_to_i64(value: u64, field: &str) -> Result<i64, AppError> {
    i64::try_from(value).map_err(|_| {
        storage_data_error(
            "Numeric value is too large for SQLite storage.",
            format!("{field}={value}"),
        )
    })
}

pub(crate) fn usize_to_i64(value: usize, field: &str) -> Result<i64, AppError> {
    i64::try_from(value).map_err(|_| {
        storage_data_error(
            "Numeric value is too large for SQLite storage.",
            format!("{field}={value}"),
        )
    })
}

pub(crate) fn opt_u64_to_i64(value: Option<u64>, field: &str) -> Result<Option<i64>, AppError> {
    value.map(|value| u64_to_i64(value, field)).transpose()
}

pub(crate) fn i64_to_u64(value: i64, field: &str) -> Result<u64, AppError> {
    u64::try_from(value).map_err(|_| {
        storage_data_error(
            "Stored numeric value cannot be represented by the Rust model.",
            format!("{field}={value}"),
        )
    })
}

pub(crate) fn opt_i64_to_u64(value: Option<i64>, field: &str) -> Result<Option<u64>, AppError> {
    value.map(|value| i64_to_u64(value, field)).transpose()
}

pub(crate) fn storage_data_error(
    message: impl Into<String>,
    details: impl Into<String>,
) -> AppError {
    AppError::with_details("DATABASE_ERROR", message, true, details)
}

fn close_behavior_label(behavior: &CloseBehavior) -> &'static str {
    match behavior {
        CloseBehavior::Ask => "ask",
        CloseBehavior::HideToTray => "hide_to_tray",
        CloseBehavior::Quit => "quit",
    }
}

fn close_behavior_from_label(value: &str) -> Result<CloseBehavior, AppError> {
    match value {
        "ask" => Ok(CloseBehavior::Ask),
        "hide_to_tray" => Ok(CloseBehavior::HideToTray),
        "quit" => Ok(CloseBehavior::Quit),
        other => Err(storage_data_error(
            "Stored close behavior is not recognized.",
            other,
        )),
    }
}

pub(crate) fn rule_mode_label(mode: &RuleMode) -> &'static str {
    match mode {
        RuleMode::PreviewOnly => "preview_only",
        RuleMode::AskFirst => "ask_first",
        RuleMode::Automatic => "automatic",
    }
}

pub(crate) fn rule_mode_from_label(value: &str) -> Result<RuleMode, AppError> {
    match value {
        "preview_only" => Ok(RuleMode::PreviewOnly),
        "ask_first" => Ok(RuleMode::AskFirst),
        "automatic" => Ok(RuleMode::Automatic),
        other => Err(storage_data_error(
            "Stored rule mode is not recognized.",
            other,
        )),
    }
}

pub(crate) fn rule_action_parts(action: &RuleAction) -> (&'static str, Option<&str>, Option<&str>) {
    match action {
        RuleAction::Trash => ("trash", None, None),
        RuleAction::Move {
            destination_folder,
            rename_template,
        } => (
            "move",
            Some(destination_folder.as_str()),
            rename_template.as_deref(),
        ),
        RuleAction::Ignore => ("ignore", None, None),
    }
}

pub(crate) fn rule_action_from_parts(
    kind: &str,
    destination_folder: Option<String>,
    rename_template: Option<String>,
) -> Result<RuleAction, AppError> {
    match kind {
        "trash" => Ok(RuleAction::Trash),
        "move" => {
            let Some(destination_folder) = destination_folder else {
                return Err(storage_data_error(
                    "Stored move rule is missing its destination folder.",
                    kind,
                ));
            };
            Ok(RuleAction::Move {
                destination_folder,
                rename_template,
            })
        }
        "ignore" => Ok(RuleAction::Ignore),
        other => Err(storage_data_error(
            "Stored rule action is not recognized.",
            other,
        )),
    }
}
