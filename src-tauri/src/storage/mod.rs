pub mod audit;
pub mod rules;
#[cfg(test)]
pub mod test_util;
pub mod tracked;

use std::path::Path;
use std::sync::Arc;

use redb::{Database, ReadableDatabase, TableDefinition};

use crate::models::{AppConfig, AppError};

pub const META_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("meta");
pub const RULES_BY_ID_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("rules_by_id");
pub const TRACKED_BY_PATH_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("tracked_by_path");
pub const TRACKED_BY_EXPIRY_TABLE: TableDefinition<u64, &[u8]> =
    TableDefinition::new("tracked_by_expiry");
pub const TRACKED_BY_RULE_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("tracked_by_rule");
pub const AUDIT_BY_SEQUENCE_TABLE: TableDefinition<u64, &[u8]> =
    TableDefinition::new("audit_by_sequence");
pub const AUDIT_BY_TIME_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("audit_by_time");

const CONFIG_KEY: &str = "config";

pub fn open_database(path: impl AsRef<Path>) -> Result<Arc<Database>, AppError> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db = Arc::new(Database::create(path)?);
    initialize_tables(&db)?;
    Ok(db)
}

pub fn initialize_tables(db: &Database) -> Result<(), AppError> {
    let write_txn = db.begin_write()?;
    {
        write_txn.open_table(META_TABLE)?;
        write_txn.open_table(RULES_BY_ID_TABLE)?;
        write_txn.open_table(TRACKED_BY_PATH_TABLE)?;
        write_txn.open_table(TRACKED_BY_EXPIRY_TABLE)?;
        write_txn.open_table(TRACKED_BY_RULE_TABLE)?;
        write_txn.open_table(AUDIT_BY_SEQUENCE_TABLE)?;
        write_txn.open_table(AUDIT_BY_TIME_TABLE)?;
    }
    write_txn.commit()?;
    Ok(())
}

pub fn get_config(db: &Database) -> Result<AppConfig, AppError> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(META_TABLE)?;
    let Some(value) = table.get(CONFIG_KEY)? else {
        return Ok(AppConfig::default());
    };

    deserialize_config(value.value())
}

pub fn save_config(db: &Database, config: &AppConfig) -> Result<(), AppError> {
    let bytes = bincode::serialize(config)?;
    let write_txn = db.begin_write()?;
    {
        let mut table = write_txn.open_table(META_TABLE)?;
        table.insert(CONFIG_KEY, bytes.as_slice())?;
    }
    write_txn.commit()?;
    Ok(())
}

fn deserialize_config(bytes: &[u8]) -> Result<AppConfig, AppError> {
    Ok(bincode::deserialize::<AppConfig>(bytes)?)
}
