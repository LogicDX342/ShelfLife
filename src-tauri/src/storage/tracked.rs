use redb::{Database, ReadableTable};

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
    let bytes = bincode::serialize(file)?;
    let write_txn = db.begin_write()?;
    {
        let mut primary = write_txn.open_table(TRACKED_BY_PATH_TABLE)?;
        primary.insert(file.path.as_str(), bytes.as_slice())?;
    }
    write_txn.commit()?;

    rebuild_tracked_indexes(db)
}

pub fn remove_tracked_file(db: &Database, path: &str) -> Result<(), AppError> {
    let write_txn = db.begin_write()?;
    {
        let mut table = write_txn.open_table(TRACKED_BY_PATH_TABLE)?;
        table.remove(path)?;
    }
    write_txn.commit()?;

    rebuild_tracked_indexes(db)
}

/// Write all files in a single transaction then rebuild indexes once.
/// Use this during reconciliation to avoid N separate disk flushes.
pub fn upsert_tracked_files_batch(db: &Database, files: &[TrackedFile]) -> Result<(), AppError> {
    if files.is_empty() {
        return Ok(());
    }
    let write_txn = db.begin_write()?;
    {
        let mut table = write_txn.open_table(TRACKED_BY_PATH_TABLE)?;
        for file in files {
            let bytes = bincode::serialize(file)?;
            table.insert(file.path.as_str(), bytes.as_slice())?;
        }
    }
    write_txn.commit()?;
    rebuild_tracked_indexes(db)
}

/// Remove all given paths in a single transaction then rebuild indexes once.
pub fn remove_tracked_files_batch(db: &Database, paths: &[&str]) -> Result<(), AppError> {
    if paths.is_empty() {
        return Ok(());
    }
    let write_txn = db.begin_write()?;
    {
        let mut table = write_txn.open_table(TRACKED_BY_PATH_TABLE)?;
        for path in paths {
            table.remove(*path)?;
        }
    }
    write_txn.commit()?;
    rebuild_tracked_indexes(db)
}

pub fn rebuild_tracked_indexes(db: &Database) -> Result<(), AppError> {
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
