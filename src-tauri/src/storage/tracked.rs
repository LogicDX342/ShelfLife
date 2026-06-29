use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::{Integer, Text};
use diesel::sqlite::SqliteConnection;

use crate::models::{AppError, Expiry, FileDecayState, OriginEvidence, TrackedFile};
use crate::storage::schema::tracked_files;
use crate::storage::{
    delete_owner_rows, i64_to_u64, insert_ordered_values, load_ordered_values,
    load_ordered_values_by_owner, opt_i64_to_u64, opt_u64_to_i64, storage_data_error, u64_to_i64,
    Database,
};

pub fn get_tracked_file(db: &Database, path: &str) -> Result<Option<TrackedFile>, AppError> {
    let mut conn = db.connect()?;
    let row = tracked_files::table
        .find(path)
        .select(TrackedRow::as_select())
        .first::<TrackedRow>(&mut conn)
        .optional()?;

    row.map(|row| {
        let matched_rule_ids = load_ordered_values(
            &mut conn,
            "tracked_file_rules",
            "file_path",
            &row.path,
            "rule_id",
        )?;
        let origin_values =
            load_ordered_values(&mut conn, "origin_values", "file_path", &row.path, "value")?;
        tracked_file_from_row(row, matched_rule_ids, origin_values)
    })
    .transpose()
}

pub fn list_tracked_files(db: &Database) -> Result<Vec<TrackedFile>, AppError> {
    let mut conn = db.connect()?;
    let rows = tracked_files::table
        .order((
            sql::<Integer>(
                "
                CASE state
                    WHEN 'decaying' THEN 0
                    WHEN 'stale' THEN 1
                    WHEN 'fresh' THEN 2
                    WHEN 'pinned' THEN 3
                    WHEN 'ignored' THEN 4
                    WHEN 'missing' THEN 5
                    ELSE 6
                END
                ",
            ),
            sql::<Text>("file_name COLLATE NOCASE ASC"),
            tracked_files::path.asc(),
        ))
        .select(TrackedRow::as_select())
        .load::<TrackedRow>(&mut conn)?;

    let mut matched_rules_by_path =
        load_ordered_values_by_owner(&mut conn, "tracked_file_rules", "file_path", "rule_id")?;
    let mut origin_values_by_path =
        load_ordered_values_by_owner(&mut conn, "origin_values", "file_path", "value")?;
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

    db.write(|conn| {
        for path in &removes {
            diesel::delete(tracked_files::table.filter(tracked_files::path.eq(path)))
                .execute(conn)?;
        }
        for file in &upserts {
            upsert_tracked_file_tx(conn, file)?;
        }
        Ok(())
    })
}

fn upsert_tracked_file_tx(conn: &mut SqliteConnection, file: &TrackedFile) -> Result<(), AppError> {
    let (expiry_kind, expires_at) = expiry_parts(&file.expiry)?;
    let origin = origin_parts(&file.origin)?;
    let row = TrackedWriteRow {
        path: &file.path,
        file_name: &file.file_name,
        watch_target_id: &file.watch_target_id,
        size_bytes: u64_to_i64(file.size_bytes, "tracked_files.size_bytes")?,
        first_seen_at: u64_to_i64(file.first_seen_at, "tracked_files.first_seen_at")?,
        last_observed_mtime: opt_u64_to_i64(
            file.last_observed_mtime,
            "tracked_files.last_observed_mtime",
        )?,
        last_observed_atime: opt_u64_to_i64(
            file.last_observed_atime,
            "tracked_files.last_observed_atime",
        )?,
        last_user_action_at: opt_u64_to_i64(
            file.last_user_action_at,
            "tracked_files.last_user_action_at",
        )?,
        freshness_at: u64_to_i64(file.freshness_at, "tracked_files.freshness_at")?,
        expiry_kind,
        expires_at,
        state: state_label(&file.state),
        origin_kind: origin.kind,
        origin_zone_id: origin.zone_id,
        origin_host_url: origin.host_url,
        origin_referrer_url: origin.referrer_url,
        origin_xattr_key: origin.xattr_key,
        origin_xattr_value_utf8: origin.xattr_value_utf8,
    };

    diesel::insert_into(tracked_files::table)
        .values(&row)
        .on_conflict(tracked_files::path)
        .do_update()
        .set(&row)
        .execute(conn)?;

    delete_owner_rows(conn, "tracked_file_rules", "file_path", &file.path)?;
    insert_ordered_values(
        conn,
        "tracked_file_rules",
        "file_path",
        "rule_id",
        &file.path,
        &file.matched_rule_ids,
        "tracked_file_rules.ordinal",
    )?;

    delete_owner_rows(conn, "origin_values", "file_path", &file.path)?;
    if let OriginEvidence::MacWhereFroms { values } = &file.origin {
        insert_ordered_values(
            conn,
            "origin_values",
            "file_path",
            "value",
            &file.path,
            values,
            "origin_values.ordinal",
        )?;
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

#[derive(Queryable, Selectable)]
#[diesel(table_name = tracked_files)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
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

#[derive(Insertable, AsChangeset)]
#[diesel(table_name = tracked_files)]
#[diesel(treat_none_as_null = true)]
struct TrackedWriteRow<'a> {
    path: &'a str,
    file_name: &'a str,
    watch_target_id: &'a str,
    size_bytes: i64,
    first_seen_at: i64,
    last_observed_mtime: Option<i64>,
    last_observed_atime: Option<i64>,
    last_user_action_at: Option<i64>,
    freshness_at: i64,
    expiry_kind: &'a str,
    expires_at: Option<i64>,
    state: &'a str,
    origin_kind: &'a str,
    origin_zone_id: Option<i64>,
    origin_host_url: Option<&'a str>,
    origin_referrer_url: Option<&'a str>,
    origin_xattr_key: Option<&'a str>,
    origin_xattr_value_utf8: Option<&'a str>,
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
    use diesel::prelude::*;

    use crate::models::{Expiry, OriginEvidence};
    use crate::storage::schema::{tracked_file_rules, tracked_files};
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
        let mut conn = fixture.db.connect().expect("database should connect");
        tracked_files::table
            .filter(tracked_files::expires_at.eq(Some(expires_at as i64)))
            .order(tracked_files::path.asc())
            .select(tracked_files::path)
            .load::<String>(&mut conn)
            .expect("expiry query should work")
    }

    fn rule_index_paths(fixture: &Fixture, rule_id: &str) -> Vec<String> {
        let mut conn = fixture.db.connect().expect("database should connect");
        tracked_file_rules::table
            .filter(tracked_file_rules::rule_id.eq(rule_id))
            .order(tracked_file_rules::file_path.asc())
            .select(tracked_file_rules::file_path)
            .load::<String>(&mut conn)
            .expect("rule query should work")
    }
}
