use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::models::{AppError, Expiry, FileDecayState, OriginEvidence, TrackedFile};
use crate::storage::{
    i64_to_u64, insert_ordered_values, load_ordered_values_by_owner, opt_i64_to_u64,
    opt_u64_to_i64, storage_data_error, u64_to_i64, usize_to_i64, Database,
};

const TRACKED_LIST_ORDER: &str = "
ORDER BY CASE state
    WHEN 'decaying' THEN 0
    WHEN 'stale' THEN 1
    WHEN 'fresh' THEN 2
    WHEN 'pinned' THEN 3
    WHEN 'ignored' THEN 4
    WHEN 'missing' THEN 5
    ELSE 6
END,
file_name COLLATE NOCASE ASC,
path ASC
";

pub fn get_tracked_file(db: &Database, path: &str) -> Result<Option<TrackedFile>, AppError> {
    let conn = db.connect()?;
    let row = conn
        .query_row(
            tracked_select_sql("WHERE path = ?1").as_str(),
            params![path],
            tracked_row_from_sql,
        )
        .optional()?;

    row.map(|row| {
        let matched_rule_ids = load_matched_rule_ids(&conn, &row.path)?;
        let origin_values = load_origin_values(&conn, &row.path)?;
        tracked_file_from_row(row, matched_rule_ids, origin_values)
    })
    .transpose()
}

pub fn list_tracked_files(db: &Database) -> Result<Vec<TrackedFile>, AppError> {
    let conn = db.connect()?;
    let rows = {
        let mut stmt = conn.prepare(tracked_select_sql(TRACKED_LIST_ORDER).as_str())?;
        let rows = stmt.query_map([], tracked_row_from_sql)?;
        let mut rows_vec = Vec::new();
        for row in rows {
            rows_vec.push(row?);
        }
        rows_vec
    };
    let mut matched_rules_by_path =
        load_ordered_values_by_owner(&conn, "tracked_file_rules", "file_path", "rule_id")?;
    let mut origin_values_by_path =
        load_ordered_values_by_owner(&conn, "origin_values", "file_path", "value")?;
    let mut files = Vec::new();
    for row in rows {
        let matched_rule_ids = matched_rules_by_path.remove(&row.path).unwrap_or_default();
        let origin_values = origin_values_by_path.remove(&row.path).unwrap_or_default();
        files.push(tracked_file_from_row(row, matched_rule_ids, origin_values)?);
    }
    Ok(files)
}

pub fn upsert_tracked_file(db: &Database, file: &TrackedFile) -> Result<(), AppError> {
    update_tracked_files_batch(db, vec![file.clone()], Vec::new())
}

pub fn replace_tracked_file(
    db: &Database,
    original_path: &str,
    file: &TrackedFile,
) -> Result<(), AppError> {
    let removes = if original_path == file.path {
        Vec::new()
    } else {
        vec![original_path.to_string()]
    };

    update_tracked_files_batch(db, vec![file.clone()], removes)
}

/// Write all file changes in a single transaction.
pub fn upsert_tracked_files_batch(db: &Database, files: &[TrackedFile]) -> Result<(), AppError> {
    update_tracked_files_batch(db, files.to_vec(), Vec::new())
}

pub fn update_tracked_files_batch(
    db: &Database,
    upserts: Vec<TrackedFile>,
    removes: Vec<String>,
) -> Result<(), AppError> {
    if upserts.is_empty() && removes.is_empty() {
        return Ok(());
    }

    db.write(|tx| {
        for path in &removes {
            tx.execute("DELETE FROM tracked_files WHERE path = ?1", params![path])?;
        }
        for file in &upserts {
            upsert_tracked_file_tx(tx, file)?;
        }
        Ok(())
    })
}

fn upsert_tracked_file_tx(tx: &Transaction<'_>, file: &TrackedFile) -> Result<(), AppError> {
    let (expiry_kind, expires_at) = expiry_parts(&file.expiry)?;
    let origin = origin_parts(&file.origin)?;

    tx.execute(
        "
        INSERT INTO tracked_files (
            path,
            file_name,
            watch_target_id,
            size_bytes,
            first_seen_at,
            last_observed_mtime,
            last_observed_atime,
            last_user_action_at,
            freshness_at,
            expiry_kind,
            expires_at,
            state,
            origin_kind,
            origin_zone_id,
            origin_host_url,
            origin_referrer_url,
            origin_xattr_key,
            origin_xattr_value_utf8
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
        ON CONFLICT(path) DO UPDATE SET
            file_name = excluded.file_name,
            watch_target_id = excluded.watch_target_id,
            size_bytes = excluded.size_bytes,
            first_seen_at = excluded.first_seen_at,
            last_observed_mtime = excluded.last_observed_mtime,
            last_observed_atime = excluded.last_observed_atime,
            last_user_action_at = excluded.last_user_action_at,
            freshness_at = excluded.freshness_at,
            expiry_kind = excluded.expiry_kind,
            expires_at = excluded.expires_at,
            state = excluded.state,
            origin_kind = excluded.origin_kind,
            origin_zone_id = excluded.origin_zone_id,
            origin_host_url = excluded.origin_host_url,
            origin_referrer_url = excluded.origin_referrer_url,
            origin_xattr_key = excluded.origin_xattr_key,
            origin_xattr_value_utf8 = excluded.origin_xattr_value_utf8
        ",
        params![
            &file.path,
            &file.file_name,
            &file.watch_target_id,
            u64_to_i64(file.size_bytes, "tracked_files.size_bytes")?,
            u64_to_i64(file.first_seen_at, "tracked_files.first_seen_at")?,
            opt_u64_to_i64(
                file.last_observed_mtime,
                "tracked_files.last_observed_mtime"
            )?,
            opt_u64_to_i64(
                file.last_observed_atime,
                "tracked_files.last_observed_atime"
            )?,
            opt_u64_to_i64(
                file.last_user_action_at,
                "tracked_files.last_user_action_at"
            )?,
            u64_to_i64(file.freshness_at, "tracked_files.freshness_at")?,
            expiry_kind,
            expires_at,
            state_label(&file.state),
            origin.kind,
            origin.zone_id,
            origin.host_url,
            origin.referrer_url,
            origin.xattr_key,
            origin.xattr_value_utf8,
        ],
    )?;

    tx.execute(
        "DELETE FROM tracked_file_rules WHERE file_path = ?1",
        params![&file.path],
    )?;
    for (ordinal, rule_id) in file.matched_rule_ids.iter().enumerate() {
        tx.execute(
            "
            INSERT INTO tracked_file_rules (file_path, ordinal, rule_id)
            VALUES (?1, ?2, ?3)
            ",
            params![
                &file.path,
                usize_to_i64(ordinal, "tracked_file_rules.ordinal")?,
                rule_id,
            ],
        )?;
    }

    tx.execute(
        "DELETE FROM origin_values WHERE file_path = ?1",
        params![&file.path],
    )?;
    if let OriginEvidence::MacWhereFroms { values } = &file.origin {
        insert_ordered_values(tx, "origin_values", "file_path", &file.path, values)?;
    }

    Ok(())
}

fn tracked_file_from_row(
    row: TrackedRow,
    matched_rule_ids: Vec<String>,
    origin_values: Vec<String>,
) -> Result<TrackedFile, AppError> {
    let origin = origin_from_row(&row, origin_values)?;

    Ok(TrackedFile {
        path: row.path,
        file_name: row.file_name,
        watch_target_id: row.watch_target_id,
        size_bytes: i64_to_u64(row.size_bytes, "tracked_files.size_bytes")?,
        first_seen_at: i64_to_u64(row.first_seen_at, "tracked_files.first_seen_at")?,
        last_observed_mtime: opt_i64_to_u64(
            row.last_observed_mtime,
            "tracked_files.last_observed_mtime",
        )?,
        last_observed_atime: opt_i64_to_u64(
            row.last_observed_atime,
            "tracked_files.last_observed_atime",
        )?,
        last_user_action_at: opt_i64_to_u64(
            row.last_user_action_at,
            "tracked_files.last_user_action_at",
        )?,
        freshness_at: i64_to_u64(row.freshness_at, "tracked_files.freshness_at")?,
        expiry: expiry_from_parts(&row.expiry_kind, row.expires_at)?,
        state: state_from_label(&row.state)?,
        matched_rule_ids,
        origin,
    })
}

fn tracked_select_sql(predicate: &str) -> String {
    format!(
        "
        SELECT path,
               file_name,
               watch_target_id,
               size_bytes,
               first_seen_at,
               last_observed_mtime,
               last_observed_atime,
               last_user_action_at,
               freshness_at,
               expiry_kind,
               expires_at,
               state,
               origin_kind,
               origin_zone_id,
               origin_host_url,
               origin_referrer_url,
               origin_xattr_key,
               origin_xattr_value_utf8
        FROM tracked_files
        {predicate}
        "
    )
}

fn tracked_row_from_sql(row: &Row<'_>) -> rusqlite::Result<TrackedRow> {
    Ok(TrackedRow {
        path: row.get(0)?,
        file_name: row.get(1)?,
        watch_target_id: row.get(2)?,
        size_bytes: row.get(3)?,
        first_seen_at: row.get(4)?,
        last_observed_mtime: row.get(5)?,
        last_observed_atime: row.get(6)?,
        last_user_action_at: row.get(7)?,
        freshness_at: row.get(8)?,
        expiry_kind: row.get(9)?,
        expires_at: row.get(10)?,
        state: row.get(11)?,
        origin_kind: row.get(12)?,
        origin_zone_id: row.get(13)?,
        origin_host_url: row.get(14)?,
        origin_referrer_url: row.get(15)?,
        origin_xattr_key: row.get(16)?,
        origin_xattr_value_utf8: row.get(17)?,
    })
}

struct TrackedRow {
    path: String,
    file_name: String,
    watch_target_id: String,
    size_bytes: i64,
    first_seen_at: i64,
    last_observed_mtime: Option<i64>,
    last_observed_atime: Option<i64>,
    last_user_action_at: Option<i64>,
    freshness_at: i64,
    expiry_kind: String,
    expires_at: Option<i64>,
    state: String,
    origin_kind: String,
    origin_zone_id: Option<i64>,
    origin_host_url: Option<String>,
    origin_referrer_url: Option<String>,
    origin_xattr_key: Option<String>,
    origin_xattr_value_utf8: Option<String>,
}

fn load_matched_rule_ids(conn: &Connection, path: &str) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare(
        "
        SELECT rule_id
        FROM tracked_file_rules
        WHERE file_path = ?1
        ORDER BY ordinal ASC
        ",
    )?;
    let rows = stmt.query_map(params![path], |row| row.get::<_, String>(0))?;
    let mut rule_ids = Vec::new();
    for row in rows {
        rule_ids.push(row?);
    }
    Ok(rule_ids)
}

fn load_origin_values(conn: &Connection, path: &str) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare(
        "
        SELECT value
        FROM origin_values
        WHERE file_path = ?1
        ORDER BY ordinal ASC
        ",
    )?;
    let rows = stmt.query_map(params![path], |row| row.get::<_, String>(0))?;
    let mut values = Vec::new();
    for row in rows {
        values.push(row?);
    }
    Ok(values)
}

fn expiry_parts(expiry: &Expiry) -> Result<(&'static str, Option<i64>), AppError> {
    match expiry {
        Expiry::At(expires_at) => Ok((
            "at",
            Some(u64_to_i64(*expires_at, "tracked_files.expires_at")?),
        )),
        Expiry::Permanent => Ok(("permanent", None)),
        Expiry::SnoozedUntil(expires_at) => Ok((
            "snoozed_until",
            Some(u64_to_i64(*expires_at, "tracked_files.expires_at")?),
        )),
    }
}

fn expiry_from_parts(kind: &str, expires_at: Option<i64>) -> Result<Expiry, AppError> {
    match kind {
        "at" => Ok(Expiry::At(i64_to_u64(
            expires_at.ok_or_else(|| {
                storage_data_error("Stored expiry is missing its timestamp.", kind)
            })?,
            "tracked_files.expires_at",
        )?)),
        "permanent" => Ok(Expiry::Permanent),
        "snoozed_until" => Ok(Expiry::SnoozedUntil(i64_to_u64(
            expires_at.ok_or_else(|| {
                storage_data_error("Stored snoozed expiry is missing its timestamp.", kind)
            })?,
            "tracked_files.expires_at",
        )?)),
        other => Err(storage_data_error(
            "Stored expiry is not recognized.",
            other,
        )),
    }
}

fn state_label(state: &FileDecayState) -> &'static str {
    match state {
        FileDecayState::Fresh => "fresh",
        FileDecayState::Stale => "stale",
        FileDecayState::Decaying => "decaying",
        FileDecayState::Pinned => "pinned",
        FileDecayState::Ignored => "ignored",
        FileDecayState::Missing => "missing",
    }
}

fn state_from_label(value: &str) -> Result<FileDecayState, AppError> {
    match value {
        "fresh" => Ok(FileDecayState::Fresh),
        "stale" => Ok(FileDecayState::Stale),
        "decaying" => Ok(FileDecayState::Decaying),
        "pinned" => Ok(FileDecayState::Pinned),
        "ignored" => Ok(FileDecayState::Ignored),
        "missing" => Ok(FileDecayState::Missing),
        other => Err(storage_data_error(
            "Stored file decay state is not recognized.",
            other,
        )),
    }
}

struct OriginParts<'a> {
    kind: &'static str,
    zone_id: Option<i64>,
    host_url: Option<&'a str>,
    referrer_url: Option<&'a str>,
    xattr_key: Option<&'a str>,
    xattr_value_utf8: Option<&'a str>,
}

fn origin_parts(origin: &OriginEvidence) -> Result<OriginParts<'_>, AppError> {
    match origin {
        OriginEvidence::MacWhereFroms { .. } => Ok(OriginParts {
            kind: "mac_where_froms",
            zone_id: None,
            host_url: None,
            referrer_url: None,
            xattr_key: None,
            xattr_value_utf8: None,
        }),
        OriginEvidence::WindowsZoneIdentifier {
            zone_id,
            host_url,
            referrer_url,
        } => Ok(OriginParts {
            kind: "windows_zone_identifier",
            zone_id: zone_id.map(i64::from),
            host_url: host_url.as_deref(),
            referrer_url: referrer_url.as_deref(),
            xattr_key: None,
            xattr_value_utf8: None,
        }),
        OriginEvidence::LinuxXattr { key, value_utf8 } => Ok(OriginParts {
            kind: "linux_xattr",
            zone_id: None,
            host_url: None,
            referrer_url: None,
            xattr_key: Some(key.as_str()),
            xattr_value_utf8: value_utf8.as_deref(),
        }),
        OriginEvidence::Unknown => Ok(OriginParts {
            kind: "unknown",
            zone_id: None,
            host_url: None,
            referrer_url: None,
            xattr_key: None,
            xattr_value_utf8: None,
        }),
    }
}

fn origin_from_row(row: &TrackedRow, values: Vec<String>) -> Result<OriginEvidence, AppError> {
    match row.origin_kind.as_str() {
        "mac_where_froms" => Ok(OriginEvidence::MacWhereFroms { values }),
        "windows_zone_identifier" => Ok(OriginEvidence::WindowsZoneIdentifier {
            zone_id: row.origin_zone_id.map(i64_to_u32).transpose()?,
            host_url: row.origin_host_url.clone(),
            referrer_url: row.origin_referrer_url.clone(),
        }),
        "linux_xattr" => {
            let Some(key) = row.origin_xattr_key.clone() else {
                return Err(storage_data_error(
                    "Stored Linux xattr origin is missing its key.",
                    row.origin_kind.clone(),
                ));
            };
            Ok(OriginEvidence::LinuxXattr {
                key,
                value_utf8: row.origin_xattr_value_utf8.clone(),
            })
        }
        "unknown" => Ok(OriginEvidence::Unknown),
        other => Err(storage_data_error(
            "Stored origin evidence is not recognized.",
            other,
        )),
    }
}

fn i64_to_u32(value: i64) -> Result<u32, AppError> {
    u32::try_from(value).map_err(|_| {
        storage_data_error(
            "Stored origin zone id cannot be represented by the Rust model.",
            format!("origin_zone_id={value}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use crate::models::{Expiry, OriginEvidence};
    use crate::storage::test_util::{path_string, Fixture};

    use super::{get_tracked_file, replace_tracked_file, update_tracked_files_batch};

    #[test]
    fn update_tracked_files_batch_updates_primary_rows_and_indexes() {
        let fixture = Fixture::new("shelflife-tracked-changes");
        let removed_path = fixture.write_watch_file("old.zip", "old");
        let kept_path = fixture.write_watch_file("new.zip", "new");
        fixture.track_file(&removed_path);
        fixture.track_file(&kept_path);

        let removed_key = path_string(&removed_path);
        let kept_key = path_string(&kept_path);

        let mut removed = get_tracked_file(&fixture.db, &removed_key)
            .expect("tracked lookup should work")
            .expect("removed fixture should be tracked");
        removed.expiry = Expiry::At(100);
        removed.matched_rule_ids = vec![String::from("old-rule")];
        super::upsert_tracked_file(&fixture.db, &removed).expect("removed fixture should save");

        let mut kept = get_tracked_file(&fixture.db, &kept_key)
            .expect("tracked lookup should work")
            .expect("kept fixture should be tracked");
        kept.expiry = Expiry::At(200);
        kept.matched_rule_ids = vec![String::from("new-rule")];

        update_tracked_files_batch(&fixture.db, vec![kept], vec![removed_key.clone()])
            .expect("changes should apply");

        assert!(get_tracked_file(&fixture.db, &removed_key)
            .expect("tracked lookup should work")
            .is_none());
        assert!(get_tracked_file(&fixture.db, &kept_key)
            .expect("tracked lookup should work")
            .is_some());
        assert_eq!(expiry_index_paths(&fixture, 100), Vec::<String>::new());
        assert_eq!(rule_index_paths(&fixture, "old-rule"), Vec::<String>::new());
        assert_eq!(expiry_index_paths(&fixture, 200), vec![kept_key.clone()]);
        assert_eq!(rule_index_paths(&fixture, "new-rule"), vec![kept_key]);
    }

    #[test]
    fn replace_tracked_file_removes_old_path_and_rebuilds_indexes_once() {
        let fixture = Fixture::new("shelflife-tracked-replace");
        let old_path = fixture.write_watch_file("old.zip", "old");
        let new_path = fixture.write_watch_file("new.zip", "new");
        fixture.track_file(&old_path);

        let old_key = path_string(&old_path);
        let new_key = path_string(&new_path);
        let mut tracked = get_tracked_file(&fixture.db, &old_key)
            .expect("tracked lookup should work")
            .expect("old fixture should be tracked");
        tracked.path = new_key.clone();
        tracked.file_name = String::from("new.zip");
        tracked.expiry = Expiry::At(300);
        tracked.matched_rule_ids = vec![String::from("replace-rule")];

        replace_tracked_file(&fixture.db, &old_key, &tracked).expect("replace should work");

        assert!(get_tracked_file(&fixture.db, &old_key)
            .expect("tracked lookup should work")
            .is_none());
        assert!(get_tracked_file(&fixture.db, &new_key)
            .expect("tracked lookup should work")
            .is_some());
        assert_eq!(rule_index_paths(&fixture, "replace-rule"), vec![new_key]);
    }

    #[test]
    fn update_tracked_files_batch_accepts_empty_changes() {
        let fixture = Fixture::new("shelflife-tracked-empty");

        update_tracked_files_batch(&fixture.db, Vec::new(), Vec::new())
            .expect("empty changes should be accepted");
    }

    #[test]
    fn tracked_file_round_trips_origin_and_expiry_variants() {
        let fixture = Fixture::new("shelflife-tracked-round-trip");
        let path = fixture.write_watch_file("source.zip", "body");
        fixture.track_file(&path);
        let key = path_string(&path);
        let mut tracked = get_tracked_file(&fixture.db, &key)
            .expect("tracked lookup should work")
            .expect("tracked file should exist");
        tracked.expiry = Expiry::SnoozedUntil(900);
        tracked.origin = OriginEvidence::WindowsZoneIdentifier {
            zone_id: Some(3),
            host_url: Some(String::from("https://example.com/file.zip")),
            referrer_url: Some(String::from("https://example.com")),
        };
        tracked.matched_rule_ids = vec![String::from("rule-a"), String::from("rule-b")];

        super::upsert_tracked_file(&fixture.db, &tracked).expect("tracked file should save");

        let loaded = get_tracked_file(&fixture.db, &key)
            .expect("tracked lookup should work")
            .expect("tracked file should exist");
        assert_eq!(loaded, tracked);
    }

    fn expiry_index_paths(fixture: &Fixture, expires_at: u64) -> Vec<String> {
        let conn = fixture.db.connect().expect("database should connect");
        let mut stmt = conn
            .prepare(
                "
                SELECT path
                FROM tracked_files
                WHERE expires_at = ?1
                ORDER BY path ASC
                ",
            )
            .expect("expiry query should prepare");
        let rows = stmt
            .query_map(params![expires_at as i64], |row| row.get::<_, String>(0))
            .expect("expiry query should work");
        rows.map(|row| row.expect("path row should load")).collect()
    }

    fn rule_index_paths(fixture: &Fixture, rule_id: &str) -> Vec<String> {
        let conn = fixture.db.connect().expect("database should connect");
        let mut stmt = conn
            .prepare(
                "
                SELECT file_path
                FROM tracked_file_rules
                WHERE rule_id = ?1
                ORDER BY file_path ASC
                ",
            )
            .expect("rule query should prepare");
        let rows = stmt
            .query_map(params![rule_id], |row| row.get::<_, String>(0))
            .expect("rule query should work");
        rows.map(|row| row.expect("path row should load")).collect()
    }
}
