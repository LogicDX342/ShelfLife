use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::{Integer, Text};
use diesel::sqlite::SqliteConnection;
use diesel::upsert::excluded;

use crate::models::{AppError, Expiry, FileDecayState, TrackedFile};
use crate::storage::schema::{tracked_file_rules, tracked_files};
use crate::storage::{
    delete_owner_rows, i64_to_u64, insert_ordered_values, load_ordered_values,
    load_ordered_values_by_owner, opt_i64_to_u64, opt_u64_to_i64, storage_data_error, u64_to_i64,
    Database,
};

const PRIMARY_WRITE_CHUNK_SIZE: usize = 64;
const RULE_WRITE_CHUNK_SIZE: usize = 300;
const DELETE_CHUNK_SIZE: usize = 500;

#[derive(Default)]
pub struct TrackedFileChanges {
    pub inserts: Vec<TrackedFile>,
    pub updates: Vec<TrackedFile>,
    pub removes: Vec<String>,
}

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
        tracked_file_from_row(row, matched_rule_ids)
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
                    WHEN 'manually_ignored' THEN 4
                    WHEN 'rule_ignored' THEN 4
                    ELSE 5
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
    let mut files = Vec::new();
    for row in rows {
        let matched_rule_ids = matched_rules_by_path.remove(&row.path).unwrap_or_default();
        files.push(tracked_file_from_row(row, matched_rule_ids)?);
    }
    Ok(files)
}

pub fn upsert_tracked_file(db: &Database, file: &TrackedFile) -> Result<(), AppError> {
    db.write(|conn| upsert_tracked_file_tx(conn, file))
}

pub fn replace_tracked_file(
    db: &Database,
    original_path: &str,
    file: &TrackedFile,
) -> Result<(), AppError> {
    db.write(|conn| replace_tracked_file_tx(conn, original_path, file))
}

pub(crate) fn replace_tracked_file_tx(
    conn: &mut SqliteConnection,
    original_path: &str,
    file: &TrackedFile,
) -> Result<(), AppError> {
    if original_path != file.path {
        diesel::delete(tracked_files::table.filter(tracked_files::path.eq(original_path)))
            .execute(conn)?;
    }
    upsert_tracked_file_tx(conn, file)
}

/// Write all file changes in a single transaction.
pub fn upsert_tracked_files_batch(db: &Database, files: &[TrackedFile]) -> Result<(), AppError> {
    apply_tracked_file_changes(
        db,
        TrackedFileChanges {
            updates: files.to_vec(),
            ..TrackedFileChanges::default()
        },
    )
}

pub fn apply_tracked_file_changes(
    db: &Database,
    changes: TrackedFileChanges,
) -> Result<(), AppError> {
    apply_tracked_file_changes_with_progress(db, changes, None)
}

pub fn apply_tracked_file_changes_with_progress(
    db: &Database,
    changes: TrackedFileChanges,
    progress_cb: Option<&dyn Fn(usize)>,
) -> Result<(), AppError> {
    if changes.inserts.is_empty() && changes.updates.is_empty() && changes.removes.is_empty() {
        return Ok(());
    }

    db.write(|conn| {
        let mut completed = 0;
        for paths in changes.removes.chunks(DELETE_CHUNK_SIZE) {
            diesel::delete(tracked_files::table.filter(tracked_files::path.eq_any(paths)))
                .execute(conn)?;
            completed += paths.len();
            emit_progress(progress_cb, completed);
        }

        for files in changes.inserts.chunks(PRIMARY_WRITE_CHUNK_SIZE) {
            insert_tracked_files_tx(conn, files)?;
            insert_tracked_file_rules_tx(conn, files)?;
            completed += files.len();
            emit_progress(progress_cb, completed);
        }

        for files in changes.updates.chunks(PRIMARY_WRITE_CHUNK_SIZE) {
            upsert_tracked_files_tx(conn, files)?;
            rebuild_tracked_file_rules_tx(conn, files)?;
            completed += files.len();
            emit_progress(progress_cb, completed);
        }
        Ok(())
    })
}

fn emit_progress(progress_cb: Option<&dyn Fn(usize)>, completed: usize) {
    if let Some(cb) = progress_cb {
        cb(completed);
    }
}

pub(crate) fn delete_tracked_file_tx(
    conn: &mut SqliteConnection,
    path: &str,
) -> Result<(), AppError> {
    diesel::delete(tracked_files::table.filter(tracked_files::path.eq(path))).execute(conn)?;
    Ok(())
}

pub(crate) fn upsert_tracked_file_tx(
    conn: &mut SqliteConnection,
    file: &TrackedFile,
) -> Result<(), AppError> {
    let rows = tracked_write_rows(std::slice::from_ref(file))?;
    let row = &rows[0];

    diesel::insert_into(tracked_files::table)
        .values(row)
        .on_conflict(tracked_files::path)
        .do_update()
        .set(row)
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

    Ok(())
}

fn insert_tracked_files_tx(
    conn: &mut SqliteConnection,
    files: &[TrackedFile],
) -> Result<(), AppError> {
    let rows = tracked_write_rows(files)?;
    diesel::insert_into(tracked_files::table)
        .values(&rows)
        .execute(conn)?;
    Ok(())
}

fn upsert_tracked_files_tx(
    conn: &mut SqliteConnection,
    files: &[TrackedFile],
) -> Result<(), AppError> {
    let rows = tracked_write_rows(files)?;
    diesel::insert_into(tracked_files::table)
        .values(&rows)
        .on_conflict(tracked_files::path)
        .do_update()
        .set((
            tracked_files::file_name.eq(excluded(tracked_files::file_name)),
            tracked_files::watch_target_id.eq(excluded(tracked_files::watch_target_id)),
            tracked_files::size_bytes.eq(excluded(tracked_files::size_bytes)),
            tracked_files::last_observed_mtime.eq(excluded(tracked_files::last_observed_mtime)),
            tracked_files::freshness_at.eq(excluded(tracked_files::freshness_at)),
            tracked_files::expiry_kind.eq(excluded(tracked_files::expiry_kind)),
            tracked_files::expires_at.eq(excluded(tracked_files::expires_at)),
            tracked_files::state.eq(excluded(tracked_files::state)),
            tracked_files::origin_url.eq(excluded(tracked_files::origin_url)),
        ))
        .execute(conn)?;
    Ok(())
}

fn rebuild_tracked_file_rules_tx(
    conn: &mut SqliteConnection,
    files: &[TrackedFile],
) -> Result<(), AppError> {
    let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    diesel::delete(tracked_file_rules::table.filter(tracked_file_rules::file_path.eq_any(paths)))
        .execute(conn)?;
    insert_tracked_file_rules_tx(conn, files)
}

fn insert_tracked_file_rules_tx(
    conn: &mut SqliteConnection,
    files: &[TrackedFile],
) -> Result<(), AppError> {
    let rows = tracked_rule_write_rows(files)?;
    for rows in rows.chunks(RULE_WRITE_CHUNK_SIZE) {
        diesel::insert_into(tracked_file_rules::table)
            .values(rows)
            .execute(conn)?;
    }
    Ok(())
}

fn tracked_write_rows(files: &[TrackedFile]) -> Result<Vec<TrackedWriteRow<'_>>, AppError> {
    files
        .iter()
        .map(|file| {
            let (expiry_kind, expires_at) = expiry_parts(&file.expiry)?;
            Ok(TrackedWriteRow {
                path: &file.path,
                file_name: &file.file_name,
                watch_target_id: &file.watch_target_id,
                size_bytes: u64_to_i64(file.size_bytes, "tracked_files.size_bytes")?,
                last_observed_mtime: opt_u64_to_i64(
                    file.last_observed_mtime,
                    "tracked_files.last_observed_mtime",
                )?,
                freshness_at: u64_to_i64(file.freshness_at, "tracked_files.freshness_at")?,
                expiry_kind,
                expires_at,
                state: state_label(&file.state),
                origin_url: file.origin_url.as_deref(),
            })
        })
        .collect()
}

fn tracked_rule_write_rows(
    files: &[TrackedFile],
) -> Result<Vec<TrackedRuleWriteRow<'_>>, AppError> {
    files
        .iter()
        .flat_map(|file| {
            file.matched_rule_ids
                .iter()
                .enumerate()
                .map(move |(ordinal, rule_id)| (file.path.as_str(), ordinal, rule_id.as_str()))
        })
        .map(|(file_path, ordinal, rule_id)| {
            Ok(TrackedRuleWriteRow {
                file_path,
                ordinal: u64_to_i64(ordinal as u64, "tracked_file_rules.ordinal")?,
                rule_id,
            })
        })
        .collect()
}

fn tracked_file_from_row(
    row: TrackedRow,
    matched_rule_ids: Vec<String>,
) -> Result<TrackedFile, AppError> {
    Ok(TrackedFile {
        path: row.path,
        file_name: row.file_name,
        watch_target_id: row.watch_target_id,
        size_bytes: i64_to_u64(row.size_bytes, "tracked_files.size_bytes")?,
        last_observed_mtime: opt_i64_to_u64(
            row.last_observed_mtime,
            "tracked_files.last_observed_mtime",
        )?,
        freshness_at: i64_to_u64(row.freshness_at, "tracked_files.freshness_at")?,
        expiry: expiry_from_parts(&row.expiry_kind, row.expires_at)?,
        state: state_from_label(&row.state)?,
        matched_rule_ids,
        origin_url: row.origin_url,
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
    last_observed_mtime: Option<i64>,
    freshness_at: i64,
    expiry_kind: String,
    expires_at: Option<i64>,
    state: String,
    origin_url: Option<String>,
}

#[derive(Insertable, AsChangeset)]
#[diesel(table_name = tracked_files)]
#[diesel(treat_none_as_null = true)]
#[diesel(treat_none_as_default_value = false)]
struct TrackedWriteRow<'a> {
    path: &'a str,
    file_name: &'a str,
    watch_target_id: &'a str,
    size_bytes: i64,
    last_observed_mtime: Option<i64>,
    freshness_at: i64,
    expiry_kind: &'a str,
    expires_at: Option<i64>,
    state: &'a str,
    origin_url: Option<&'a str>,
}

#[derive(Insertable)]
#[diesel(table_name = tracked_file_rules)]
struct TrackedRuleWriteRow<'a> {
    file_path: &'a str,
    ordinal: i64,
    rule_id: &'a str,
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
        FileDecayState::ManuallyIgnored => "manually_ignored",
        FileDecayState::RuleIgnored => "rule_ignored",
    }
}

fn state_from_label(value: &str) -> Result<FileDecayState, AppError> {
    match value {
        "fresh" => Ok(FileDecayState::Fresh),
        "stale" => Ok(FileDecayState::Stale),
        "decaying" => Ok(FileDecayState::Decaying),
        "pinned" => Ok(FileDecayState::Pinned),
        "manually_ignored" => Ok(FileDecayState::ManuallyIgnored),
        "rule_ignored" => Ok(FileDecayState::RuleIgnored),
        other => Err(storage_data_error(
            "Stored file decay state is not recognized.",
            other,
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use diesel::prelude::*;

    use crate::models::{Expiry, FileDecayState, TrackedFile};
    use crate::storage::schema::{tracked_file_rules, tracked_files};
    use crate::storage::test_util::{path_string, Fixture};

    use super::{
        apply_tracked_file_changes, apply_tracked_file_changes_with_progress, get_tracked_file,
        replace_tracked_file, TrackedFileChanges, PRIMARY_WRITE_CHUNK_SIZE,
    };

    #[test]
    fn apply_tracked_file_changes_updates_primary_rows_and_indexes() {
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

        apply_tracked_file_changes(
            &fixture.db,
            TrackedFileChanges {
                updates: vec![kept],
                removes: vec![removed_key.clone()],
                ..TrackedFileChanges::default()
            },
        )
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
    fn apply_tracked_file_changes_batches_new_primary_and_rule_rows() {
        let fixture = Fixture::new("shelflife-tracked-batch-insert");
        let insert_count = PRIMARY_WRITE_CHUNK_SIZE * 2 + 2;
        let inserts = (0..insert_count)
            .map(|index| tracked_for_batch(&fixture, index))
            .collect::<Vec<_>>();
        let progress = Mutex::new(Vec::new());
        let progress_callback = |completed| {
            progress
                .lock()
                .expect("progress lock should work")
                .push(completed);
        };

        apply_tracked_file_changes_with_progress(
            &fixture.db,
            TrackedFileChanges {
                inserts,
                ..TrackedFileChanges::default()
            },
            Some(&progress_callback),
        )
        .expect("batched inserts should apply");

        assert_eq!(
            *progress.lock().expect("progress lock should work"),
            vec![
                PRIMARY_WRITE_CHUNK_SIZE,
                PRIMARY_WRITE_CHUNK_SIZE * 2,
                insert_count,
            ]
        );

        let mut conn = fixture.db.connect().expect("database should connect");
        assert_eq!(
            tracked_files::table
                .count()
                .get_result::<i64>(&mut conn)
                .expect("tracked count should load"),
            insert_count as i64
        );
        assert_eq!(
            tracked_file_rules::table
                .count()
                .get_result::<i64>(&mut conn)
                .expect("rule index count should load"),
            (insert_count * 3) as i64
        );

        drop(conn);
        let updates = (0..insert_count)
            .map(|index| {
                let mut tracked = tracked_for_batch(&fixture, index);
                tracked.size_bytes += 1_000;
                tracked.matched_rule_ids = vec![format!("updated-rule-{index}")];
                tracked
            })
            .collect();
        apply_tracked_file_changes(
            &fixture.db,
            TrackedFileChanges {
                updates,
                ..TrackedFileChanges::default()
            },
        )
        .expect("batched updates should apply");

        let updated = super::list_tracked_files(&fixture.db).expect("tracked files should load");
        assert_eq!(updated.len(), insert_count);
        assert!(updated.iter().all(|file| file.size_bytes >= 1_000));
        assert!(updated.iter().all(|file| file.matched_rule_ids.len() == 1));
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
    fn apply_tracked_file_changes_accepts_empty_changes() {
        let fixture = Fixture::new("shelflife-tracked-empty");

        apply_tracked_file_changes(&fixture.db, TrackedFileChanges::default())
            .expect("empty changes should be accepted");
    }

    #[test]
    fn tracked_file_round_trips_origin_url_and_expiry_variants() {
        let fixture = Fixture::new("shelflife-tracked-round-trip");
        let path = fixture.write_watch_file("source.zip", "body");
        fixture.track_file(&path);
        let key = path_string(&path);
        let mut tracked = get_tracked_file(&fixture.db, &key)
            .expect("tracked lookup should work")
            .expect("tracked file should exist");
        tracked.expiry = Expiry::SnoozedUntil(900);
        tracked.origin_url = Some(String::from("https://example.com/"));
        tracked.matched_rule_ids = vec![String::from("rule-a"), String::from("rule-b")];

        super::upsert_tracked_file(&fixture.db, &tracked).expect("tracked file should save");

        let loaded = get_tracked_file(&fixture.db, &key)
            .expect("tracked lookup should work")
            .expect("tracked file should exist");
        assert_eq!(loaded, tracked);
    }

    fn tracked_for_batch(fixture: &Fixture, index: usize) -> TrackedFile {
        TrackedFile {
            path: path_string(&fixture.root.join(format!("batch-{index}.zip"))),
            file_name: format!("batch-{index}.zip"),
            watch_target_id: String::from("watch"),
            size_bytes: index as u64,
            last_observed_mtime: None,
            freshness_at: 1,
            expiry: Expiry::At(2),
            state: FileDecayState::Fresh,
            matched_rule_ids: (0..3).map(|rule| format!("rule-{index}-{rule}")).collect(),
            origin_url: None,
        }
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
