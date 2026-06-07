use redb::{Database, ReadableTable};

use crate::models::{AppError, AutomationRule};
use crate::storage::RULES_BY_ID_TABLE;

#[allow(dead_code)]
pub fn get_rule(db: &Database, id: &str) -> Result<Option<AutomationRule>, AppError> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(RULES_BY_ID_TABLE)?;
    let Some(value) = table.get(id)? else {
        return Ok(None);
    };

    Ok(Some(bincode::deserialize(value.value())?))
}

pub fn list_rules(db: &Database) -> Result<Vec<AutomationRule>, AppError> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(RULES_BY_ID_TABLE)?;
    let mut rules = Vec::new();

    for item in table.iter()? {
        let (_, value) = item?;
        rules.push(bincode::deserialize(value.value())?);
    }

    rules.sort_by(|a: &AutomationRule, b: &AutomationRule| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(rules)
}

pub fn save_rule(db: &Database, rule: &AutomationRule) -> Result<(), AppError> {
    let bytes = bincode::serialize(rule)?;
    let write_txn = db.begin_write()?;
    {
        let mut table = write_txn.open_table(RULES_BY_ID_TABLE)?;
        table.insert(rule.id.as_str(), bytes.as_slice())?;
    }
    write_txn.commit()?;
    Ok(())
}

pub fn delete_rule(db: &Database, id: &str) -> Result<(), AppError> {
    let write_txn = db.begin_write()?;
    {
        let mut table = write_txn.open_table(RULES_BY_ID_TABLE)?;
        table.remove(id)?;
    }
    write_txn.commit()?;
    Ok(())
}
