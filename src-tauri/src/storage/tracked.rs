use redb::{Database, ReadableDatabase, ReadableTable};

use crate::models::{AppError, Expiry, FileDecayState, TrackedFile};
use crate::storage::{TRACKED_BY_EXPIRY_TABLE, TRACKED_BY_PATH_TABLE, TRACKED_BY_RULE_TABLE};

pub fn get_tracked_file(db: &Database, path: &str) -> Result<Option<TrackedFile>, AppError> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(TRACKED_BY_PATH_TABLE)?;
    let Some(value) = table.get(path)? else {
        return Ok(None);
    };

    Ok(Some(bincode::deserialize(value.value())?))
}

pub fn list_tracked_files(db: &Database) -> Result<Vec<TrackedFile>, AppError> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(TRACKED_BY_PATH_TABLE)?;
    let mut files = Vec::new();

    for item in table.iter()? {
        let (_, value) = item?;
        files.push(bincode::deserialize(value.value())?);
    }

    files.sort_by(|a: &TrackedFile, b: &TrackedFile| {
        state_sort_key(&a.state)
            .cmp(state_sort_key(&b.state))
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
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

/// Write all files in a single transaction then rebuild indexes once.
/// Use this during reconciliation to avoid N separate disk flushes.
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

    let write_txn = db.begin_write()?;
    {
        let mut table = write_txn.open_table(TRACKED_BY_PATH_TABLE)?;
        for path in &removes {
            table.remove(path.as_str())?;
        }
        for file in &upserts {
            let bytes = bincode::serialize(file)?;
            table.insert(file.path.as_str(), bytes.as_slice())?;
        }
    }
    write_txn.commit()?;

    rebuild_tracked_indexes(db)
}

fn rebuild_tracked_indexes(db: &Database) -> Result<(), AppError> {
    let files = {
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(TRACKED_BY_PATH_TABLE)?;
        let mut files = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            files.push(bincode::deserialize::<TrackedFile>(value.value())?);
        }
        files
    };

    let write_txn = db.begin_write()?;
    {
        let mut by_expiry = write_txn.open_table(TRACKED_BY_EXPIRY_TABLE)?;
        by_expiry.retain(|_, _| false)?;

        let mut by_rule = write_txn.open_table(TRACKED_BY_RULE_TABLE)?;
        by_rule.retain(|_, _| false)?;

        for file in &files {
            if let Expiry::At(expires_at) | Expiry::SnoozedUntil(expires_at) = file.expiry {
                let mut paths = match by_expiry.get(expires_at)? {
                    Some(existing) => bincode::deserialize::<Vec<String>>(existing.value())?,
                    None => Vec::new(),
                };
                paths.push(file.path.clone());
                let bytes = bincode::serialize(&paths)?;
                by_expiry.insert(expires_at, bytes.as_slice())?;
            }

            for rule_id in &file.matched_rule_ids {
                let mut paths = match by_rule.get(rule_id.as_str())? {
                    Some(existing) => bincode::deserialize::<Vec<String>>(existing.value())?,
                    None => Vec::new(),
                };
                paths.push(file.path.clone());
                let bytes = bincode::serialize(&paths)?;
                by_rule.insert(rule_id.as_str(), bytes.as_slice())?;
            }
        }
    }
    write_txn.commit()?;
    Ok(())
}

fn state_sort_key(state: &FileDecayState) -> &'static str {
    match state {
        FileDecayState::Decaying => "0",
        FileDecayState::Stale => "1",
        FileDecayState::Fresh => "2",
        FileDecayState::Pinned => "3",
        FileDecayState::Ignored => "4",
        FileDecayState::Missing => "5",
    }
}

#[cfg(test)]
mod tests {
    use redb::ReadableDatabase;

    use crate::models::Expiry;
    use crate::storage;
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
        assert_eq!(expiry_index_paths(&fixture.db, 100), Vec::<String>::new());
        assert_eq!(
            rule_index_paths(&fixture.db, "old-rule"),
            Vec::<String>::new()
        );
        assert_eq!(expiry_index_paths(&fixture.db, 200), vec![kept_key.clone()]);
        assert_eq!(rule_index_paths(&fixture.db, "new-rule"), vec![kept_key]);
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
        assert_eq!(rule_index_paths(&fixture.db, "replace-rule"), vec![new_key]);
    }

    #[test]
    fn update_tracked_files_batch_accepts_empty_changes() {
        let fixture = Fixture::new("shelflife-tracked-empty");

        update_tracked_files_batch(&fixture.db, Vec::new(), Vec::new())
            .expect("empty changes should be accepted");
    }

    fn expiry_index_paths(db: &redb::Database, expires_at: u64) -> Vec<String> {
        let read_txn = db.begin_read().expect("read transaction should start");
        let table = read_txn
            .open_table(storage::TRACKED_BY_EXPIRY_TABLE)
            .expect("expiry table should open");
        table
            .get(expires_at)
            .expect("expiry lookup should work")
            .map(|value| bincode::deserialize(value.value()).expect("paths should deserialize"))
            .unwrap_or_default()
    }

    fn rule_index_paths(db: &redb::Database, rule_id: &str) -> Vec<String> {
        let read_txn = db.begin_read().expect("read transaction should start");
        let table = read_txn
            .open_table(storage::TRACKED_BY_RULE_TABLE)
            .expect("rule table should open");
        table
            .get(rule_id)
            .expect("rule lookup should work")
            .map(|value| bincode::deserialize(value.value()).expect("paths should deserialize"))
            .unwrap_or_default()
    }
}
