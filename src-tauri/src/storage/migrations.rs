use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Nullable, Text};
use diesel::sqlite::SqliteConnection;
use url::Url;

use crate::models::AppError;

use super::SCHEMA_VERSION;

pub(super) fn migrate(conn: &mut SqliteConnection, current_version: i64) -> Result<(), AppError> {
    conn.immediate_transaction(|conn| {
        let mut version = current_version;
        while version < SCHEMA_VERSION {
            version = match version {
                1 => {
                    migrate_v1_to_v2(conn)?;
                    2
                }
                unsupported => {
                    return Err(AppError::with_details(
                        "DATABASE_ERROR",
                        "Database schema cannot be upgraded by this ShelfLife build.",
                        true,
                        format!("database={unsupported}, supported={SCHEMA_VERSION}"),
                    ));
                }
            };
        }

        sql_query(format!("PRAGMA user_version = {SCHEMA_VERSION}")).execute(conn)?;
        Ok(())
    })
}

fn migrate_v1_to_v2(conn: &mut SqliteConnection) -> Result<(), AppError> {
    let legacy_origins = sql_query(
        "SELECT path, origin_host_url AS host_url, origin_referrer_url AS referrer_url
         FROM tracked_files
         WHERE state <> 'missing'",
    )
    .load::<LegacyOriginRow>(conn)?;

    for statement in V1_TO_V2_SQL
        .split(';')
        .map(str::trim)
        .filter(|sql| !sql.is_empty())
    {
        sql_query(statement).execute(conn)?;
    }

    for legacy in legacy_origins {
        let origin_url = canonical_origin_url(
            legacy
                .host_url
                .iter()
                .chain(legacy.referrer_url.iter())
                .map(String::as_str),
        );
        sql_query("UPDATE tracked_files SET origin_url = ? WHERE path = ?")
            .bind::<Nullable<Text>, _>(origin_url.as_deref())
            .bind::<Text, _>(&legacy.path)
            .execute(conn)?;
    }

    Ok(())
}

fn canonical_origin_url<'a>(candidates: impl IntoIterator<Item = &'a str>) -> Option<String> {
    candidates.into_iter().find_map(|candidate| {
        let mut url = Url::parse(candidate.trim()).ok()?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return None;
        }

        url.set_username("").ok()?;
        url.set_password(None).ok()?;
        url.set_path("/");
        url.set_query(None);
        url.set_fragment(None);
        Some(url.to_string())
    })
}

#[derive(QueryableByName)]
struct LegacyOriginRow {
    #[diesel(sql_type = Text)]
    path: String,
    #[diesel(sql_type = Nullable<Text>)]
    host_url: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    referrer_url: Option<String>,
}

const V1_TO_V2_SQL: &str = r#"
CREATE TABLE app_config_v2 (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    default_ttl_seconds INTEGER NOT NULL,
    stale_threshold_seconds INTEGER NOT NULL,
    decaying_threshold_seconds INTEGER NOT NULL,
    default_move_destination TEXT,
    notifications_enabled INTEGER NOT NULL CHECK (notifications_enabled IN (0, 1)),
    start_at_login INTEGER NOT NULL CHECK (start_at_login IN (0, 1)),
    close_behavior TEXT NOT NULL CHECK (close_behavior IN ('ask', 'hide_to_tray', 'quit')),
    dropzone_enabled INTEGER NOT NULL CHECK (dropzone_enabled IN (0, 1))
);

INSERT INTO app_config_v2 (
    id, default_ttl_seconds, stale_threshold_seconds, decaying_threshold_seconds,
    default_move_destination, notifications_enabled, start_at_login,
    close_behavior, dropzone_enabled
)
SELECT
    id, default_ttl_seconds, stale_threshold_seconds, decaying_threshold_seconds,
    safe_folder_path, notifications_enabled, start_at_login,
    close_behavior, dropzone_enabled
FROM app_config;

DROP TABLE app_config;
ALTER TABLE app_config_v2 RENAME TO app_config;

CREATE TABLE tracked_files_v2 (
    path TEXT PRIMARY KEY,
    file_name TEXT NOT NULL,
    watch_target_id TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    last_observed_mtime INTEGER,
    freshness_at INTEGER NOT NULL,
    expiry_kind TEXT NOT NULL CHECK (expiry_kind IN ('at', 'permanent', 'snoozed_until')),
    expires_at INTEGER,
    state TEXT NOT NULL CHECK (state IN ('fresh', 'stale', 'decaying', 'pinned', 'manually_ignored', 'rule_ignored')),
    origin_url TEXT
);

INSERT INTO tracked_files_v2 (
    path, file_name, watch_target_id, size_bytes, last_observed_mtime,
    freshness_at, expiry_kind, expires_at, state, origin_url
)
SELECT
    path, file_name, watch_target_id, size_bytes, last_observed_mtime,
    freshness_at, expiry_kind, expires_at,
    CASE
        WHEN state = 'ignored' AND last_user_action_at IS NOT NULL THEN 'manually_ignored'
        WHEN state = 'ignored' THEN 'rule_ignored'
        ELSE state
    END,
    NULL
FROM tracked_files
WHERE state <> 'missing';

CREATE TABLE tracked_file_rules_v2 (
    file_path TEXT NOT NULL REFERENCES tracked_files_v2(path) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    rule_id TEXT NOT NULL,
    PRIMARY KEY (file_path, ordinal),
    UNIQUE (file_path, rule_id)
);

INSERT INTO tracked_file_rules_v2 (file_path, ordinal, rule_id)
SELECT rules.file_path, rules.ordinal, rules.rule_id
FROM tracked_file_rules AS rules
INNER JOIN tracked_files_v2 AS files ON files.path = rules.file_path;

DROP TABLE tracked_file_rules;
DROP TABLE origin_values;
DROP TABLE IF EXISTS watch_target_include_hidden_patterns;
DROP TABLE tracked_files;
ALTER TABLE tracked_files_v2 RENAME TO tracked_files;
ALTER TABLE tracked_file_rules_v2 RENAME TO tracked_file_rules;

CREATE INDEX idx_tracked_state ON tracked_files(state);
CREATE INDEX idx_tracked_expiry ON tracked_files(expires_at);
CREATE INDEX idx_tracked_file_rules_rule_id ON tracked_file_rules(rule_id);
"#;

#[cfg(test)]
mod tests {
    use std::fs;

    use diesel::prelude::*;
    use diesel::sql_query;
    use diesel::sql_types::{Integer, Nullable, Text};
    use uuid::Uuid;

    use crate::models::FileDecayState;
    use crate::storage;

    #[test]
    fn opening_v1_database_migrates_tracked_files_and_rule_links() {
        let root = std::env::temp_dir().join(format!("shelflife-migration-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("migration fixture directory should be created");
        let database_path = root.join("test.sqlite");
        let database_url = database_path.to_string_lossy().into_owned();
        let mut conn =
            SqliteConnection::establish(&database_url).expect("legacy database should connect");

        for statement in V1_FIXTURE_SQL
            .split(';')
            .map(str::trim)
            .filter(|sql| !sql.is_empty())
        {
            sql_query(statement)
                .execute(&mut conn)
                .expect("legacy schema should be created");
        }
        drop(conn);

        let db = storage::open_database(&database_path).expect("legacy database should migrate");
        let tracked = storage::tracked::get_tracked_file(&db, "C:\\watch\\download.zip")
            .expect("tracked lookup should work")
            .expect("active tracked file should survive migration");

        assert_eq!(
            tracked.origin_url.as_deref(),
            Some("https://downloads.example.com/")
        );
        assert_eq!(tracked.matched_rule_ids, vec![String::from("rule-a")]);
        let manually_ignored =
            storage::tracked::get_tracked_file(&db, "C:\\watch\\manually-ignored.zip")
                .expect("manual ignore lookup should work")
                .expect("manually ignored file should survive migration");
        assert_eq!(manually_ignored.state, FileDecayState::ManuallyIgnored);
        let rule_ignored = storage::tracked::get_tracked_file(&db, "C:\\watch\\rule-ignored.zip")
            .expect("rule ignore lookup should work")
            .expect("rule ignored file should survive migration");
        assert_eq!(rule_ignored.state, FileDecayState::RuleIgnored);
        assert!(
            storage::tracked::get_tracked_file(&db, "C:\\watch\\missing.zip")
                .expect("missing lookup should work")
                .is_none()
        );
        let mut conn = db.connect().expect("migrated database should connect");
        let config = sql_query("SELECT default_move_destination FROM app_config WHERE id = 1")
            .get_result::<MigratedDefaultRow>(&mut conn)
            .expect("migrated config should load");
        assert_eq!(
            config.default_move_destination.as_deref(),
            Some("C:\\Users\\tester\\shelflife-safe")
        );

        storage::tracked::upsert_tracked_file(&db, &tracked)
            .expect("migrated row should use the v2 write schema");
        let version = sql_query("PRAGMA user_version")
            .get_result::<UserVersionRow>(&mut conn)
            .expect("schema version should load");
        assert_eq!(version.user_version, 2);

        drop(conn);
        drop(db);
        fs::remove_dir_all(root).expect("migration fixture should be removed");
    }

    #[test]
    fn fresh_database_has_no_default_move_destination() {
        let root =
            std::env::temp_dir().join(format!("shelflife-fresh-database-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("database directory should be created");

        let db =
            storage::open_database(root.join("test.sqlite")).expect("fresh database should open");
        let config = storage::get_config(&db).expect("fresh config should load");

        assert_eq!(config.default_move_destination, None);

        drop(db);
        fs::remove_dir_all(root).expect("database directory should be removed");
    }

    #[derive(QueryableByName)]
    struct UserVersionRow {
        #[diesel(sql_type = Integer)]
        user_version: i32,
    }

    #[derive(QueryableByName)]
    struct MigratedDefaultRow {
        #[diesel(sql_type = Nullable<Text>)]
        default_move_destination: Option<String>,
    }

    const V1_FIXTURE_SQL: &str = r#"
CREATE TABLE app_config (
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

INSERT INTO app_config (
    id, default_ttl_seconds, stale_threshold_seconds, decaying_threshold_seconds,
    safe_folder_path, notifications_enabled, start_at_login, close_behavior,
    dropzone_enabled
) VALUES (
    1, 2592000, 432000, 86400,
    'C:\Users\tester\shelflife-safe', 1, 0, 'ask', 0
);

CREATE TABLE tracked_files (
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

CREATE TABLE tracked_file_rules (
    file_path TEXT NOT NULL REFERENCES tracked_files(path) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    rule_id TEXT NOT NULL,
    PRIMARY KEY (file_path, ordinal),
    UNIQUE (file_path, rule_id)
);

CREATE TABLE origin_values (
    file_path TEXT NOT NULL REFERENCES tracked_files(path) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (file_path, ordinal)
);

CREATE INDEX idx_tracked_state ON tracked_files(state);
CREATE INDEX idx_tracked_expiry ON tracked_files(expires_at);
CREATE INDEX idx_tracked_file_rules_rule_id ON tracked_file_rules(rule_id);

INSERT INTO tracked_files (
    path, file_name, watch_target_id, size_bytes, first_seen_at, freshness_at,
    expiry_kind, expires_at, state, origin_kind, origin_host_url, origin_referrer_url
) VALUES (
    'C:\watch\download.zip', 'download.zip', 'watch', 10, 1, 2,
    'at', 3, 'fresh', 'windows_zone_identifier',
    ' https://user:secret@downloads.example.com:443/file.zip?token=1#fragment ',
    'https://referrer.example.com/path'
);

INSERT INTO tracked_files (
    path, file_name, watch_target_id, size_bytes, first_seen_at, freshness_at,
    expiry_kind, expires_at, state, origin_kind
) VALUES (
    'C:\watch\missing.zip', 'missing.zip', 'watch', 10, 1, 2,
    'at', 3, 'missing', 'unknown'
);

INSERT INTO tracked_files (
    path, file_name, watch_target_id, size_bytes, first_seen_at,
    last_user_action_at, freshness_at, expiry_kind, expires_at, state, origin_kind
) VALUES (
    'C:\watch\manually-ignored.zip', 'manually-ignored.zip', 'watch', 10, 1,
    2, 2, 'at', 3, 'ignored', 'unknown'
);

INSERT INTO tracked_files (
    path, file_name, watch_target_id, size_bytes, first_seen_at,
    freshness_at, expiry_kind, expires_at, state, origin_kind
) VALUES (
    'C:\watch\rule-ignored.zip', 'rule-ignored.zip', 'watch', 10, 1,
    2, 'at', 3, 'ignored', 'unknown'
);

INSERT INTO tracked_file_rules (file_path, ordinal, rule_id)
VALUES ('C:\watch\download.zip', 0, 'rule-a');
INSERT INTO tracked_file_rules (file_path, ordinal, rule_id)
VALUES ('C:\watch\missing.zip', 0, 'rule-missing');
PRAGMA user_version = 1;
"#;
}
