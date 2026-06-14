use redb::{Database, ReadableDatabase, ReadableTable};
use std::cmp::Reverse;

use crate::models::{AppError, AuditEntry};
use crate::storage::{AUDIT_BY_SEQUENCE_TABLE, AUDIT_BY_TIME_TABLE, META_TABLE};

const NEXT_AUDIT_SEQUENCE_KEY: &str = "next_audit_sequence";

pub fn next_audit_sequence(db: &Database) -> Result<u64, AppError> {
    let write_txn = db.begin_write()?;
    let sequence = {
        let mut meta = write_txn.open_table(META_TABLE)?;
        let next = match meta.get(NEXT_AUDIT_SEQUENCE_KEY)? {
            Some(value) => bincode::deserialize::<u64>(value.value())?,
            None => 1,
        };
        let bytes = bincode::serialize(&(next + 1))?;
        meta.insert(NEXT_AUDIT_SEQUENCE_KEY, bytes.as_slice())?;
        next
    };
    write_txn.commit()?;
    Ok(sequence)
}

pub fn get_audit_entry_by_id(db: &Database, id: &str) -> Result<Option<AuditEntry>, AppError> {
    let entries = list_audit_entries(db)?;
    Ok(entries.into_iter().find(|entry| entry.id == id))
}

pub fn list_audit_entries(db: &Database) -> Result<Vec<AuditEntry>, AppError> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(AUDIT_BY_SEQUENCE_TABLE)?;
    let mut entries: Vec<AuditEntry> = Vec::new();

    for item in table.iter()? {
        let (_, value) = item?;
        entries.push(bincode::deserialize(value.value())?);
    }

    entries.sort_by_key(|entry| Reverse(entry.sequence));
    Ok(entries)
}

pub fn append_audit_entry(db: &Database, entry: &AuditEntry) -> Result<(), AppError> {
    let bytes = bincode::serialize(entry)?;
    let write_txn = db.begin_write()?;
    {
        let mut by_sequence = write_txn.open_table(AUDIT_BY_SEQUENCE_TABLE)?;
        by_sequence.insert(entry.sequence, bytes.as_slice())?;

        let mut by_time = write_txn.open_table(AUDIT_BY_TIME_TABLE)?;
        let mut sequences = match by_time.get(entry.timestamp)? {
            Some(value) => bincode::deserialize::<Vec<u64>>(value.value())?,
            None => Vec::new(),
        };
        sequences.push(entry.sequence);
        let sequence_bytes = bincode::serialize(&sequences)?;
        by_time.insert(entry.timestamp, sequence_bytes.as_slice())?;
    }
    write_txn.commit()?;
    Ok(())
}

pub fn update_audit_entry(db: &Database, entry: &AuditEntry) -> Result<(), AppError> {
    let bytes = bincode::serialize(entry)?;
    let write_txn = db.begin_write()?;
    {
        let mut by_sequence = write_txn.open_table(AUDIT_BY_SEQUENCE_TABLE)?;
        by_sequence.insert(entry.sequence, bytes.as_slice())?;
    }
    write_txn.commit()?;
    Ok(())
}
