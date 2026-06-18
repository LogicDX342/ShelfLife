pub mod audit;
pub mod rules;
#[cfg(test)]
pub mod test_util;
pub mod tracked;

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use redb::{Database, ReadableDatabase, TableDefinition};

use crate::models::{AppConfig, AppError, CloseBehavior, WatchTarget};

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

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub watcher: Arc<Mutex<Option<crate::engine::watcher::ShelflifeDebouncer>>>,
    pub watching_paused: Arc<AtomicBool>,
    pub reconciliation_active: Arc<AtomicBool>,
    pub rule_execution_active: Arc<AtomicBool>,
    pub rule_scheduler_wake: Arc<(Mutex<bool>, Condvar)>,
    automatic_rule_retries: Arc<Mutex<HashMap<AutomaticRuleRetryKey, AutomaticRuleRetry>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AutomaticRuleRetryKey {
    path: String,
    rule_id: String,
}

#[derive(Debug, Clone)]
struct AutomaticRuleRetry {
    failure_count: u32,
    retry_after: u64,
}

impl AppState {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            watcher: Arc::new(Mutex::new(None)),
            watching_paused: Arc::new(AtomicBool::new(false)),
            reconciliation_active: Arc::new(AtomicBool::new(false)),
            rule_execution_active: Arc::new(AtomicBool::new(false)),
            rule_scheduler_wake: Arc::new((Mutex::new(false), Condvar::new())),
            automatic_rule_retries: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn is_watching_paused(&self) -> bool {
        self.watching_paused.load(Ordering::Relaxed)
    }

    pub fn set_watching_paused(&self, paused: bool) {
        self.watching_paused.store(paused, Ordering::Relaxed);
        self.wake_rule_scheduler();
    }

    pub fn wake_rule_scheduler(&self) {
        let (lock, wake) = &*self.rule_scheduler_wake;
        if let Ok(mut pending) = lock.lock() {
            *pending = true;
            wake.notify_all();
        }
    }

    pub fn wait_for_rule_scheduler_wake(&self, timeout: Option<Duration>) -> bool {
        let (lock, wake) = &*self.rule_scheduler_wake;
        let Ok(mut pending) = lock.lock() else {
            return false;
        };

        if *pending {
            *pending = false;
            return true;
        }

        match timeout {
            Some(timeout) => match wake.wait_timeout(pending, timeout) {
                Ok((mut guard, result)) => {
                    let was_woken = *guard || !result.timed_out();
                    *guard = false;
                    was_woken
                }
                Err(_) => false,
            },
            None => match wake.wait(pending) {
                Ok(mut guard) => {
                    let was_woken = *guard;
                    *guard = false;
                    was_woken
                }
                Err(_) => false,
            },
        }
    }

    pub fn automatic_rule_retry_after(&self, path: &str, rule_id: &str) -> Option<u64> {
        let retries = self.automatic_rule_retries.lock().ok()?;
        retries
            .get(&AutomaticRuleRetryKey {
                path: path.to_string(),
                rule_id: rule_id.to_string(),
            })
            .map(|retry| retry.retry_after)
    }

    pub fn record_automatic_rule_failure(&self, path: &str, rule_id: &str, now: u64) -> u64 {
        const RETRY_BACKOFF_SECONDS: [u64; 4] = [60, 5 * 60, 15 * 60, 60 * 60];

        let Ok(mut retries) = self.automatic_rule_retries.lock() else {
            return now + RETRY_BACKOFF_SECONDS[0];
        };

        let retry = retries
            .entry(AutomaticRuleRetryKey {
                path: path.to_string(),
                rule_id: rule_id.to_string(),
            })
            .or_insert(AutomaticRuleRetry {
                failure_count: 0,
                retry_after: now,
            });

        retry.failure_count = retry.failure_count.saturating_add(1);
        let backoff = RETRY_BACKOFF_SECONDS
            .get(retry.failure_count.saturating_sub(1) as usize)
            .copied()
            .unwrap_or(*RETRY_BACKOFF_SECONDS.last().unwrap_or(&60));
        retry.retry_after = now + backoff;
        retry.retry_after
    }

    pub fn clear_automatic_rule_failure(&self, path: &str, rule_id: &str) {
        if let Ok(mut retries) = self.automatic_rule_retries.lock() {
            retries.remove(&AutomaticRuleRetryKey {
                path: path.to_string(),
                rule_id: rule_id.to_string(),
            });
        }
    }
}

pub fn open_database(path: impl AsRef<Path>) -> Result<Arc<Database>, AppError> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db = Arc::new(Database::create(path)?);
    initialize_tables(&db)?;
    migrate_tracked_files(&db)?;
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

/// One-time migration: if any tracked files were serialized without the
/// `watch_target_id` field (old schema), they will fail bincode deserialization.
/// Clear those stale entries — they'll be re-indexed by the startup reconciliation.
fn migrate_tracked_files(db: &Database) -> Result<(), AppError> {
    use redb::ReadableTable;

    let stale_keys: Vec<String> = {
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(TRACKED_BY_PATH_TABLE)?;
        let mut keys = Vec::new();
        for item in table.iter()? {
            let (key, value) = item?;
            if bincode::deserialize::<crate::models::TrackedFile>(value.value()).is_err() {
                keys.push(key.value().to_string());
            }
        }
        keys
    };

    if stale_keys.is_empty() {
        return Ok(());
    }

    let write_txn = db.begin_write()?;
    {
        let mut table = write_txn.open_table(TRACKED_BY_PATH_TABLE)?;
        for key in &stale_keys {
            table.remove(key.as_str())?;
        }
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

#[derive(serde::Deserialize)]
struct LegacyAppConfig {
    version: u32,
    watch_targets: Vec<WatchTarget>,
    protected_patterns: Vec<String>,
    default_ttl_seconds: u64,
    stale_threshold_seconds: u64,
    decaying_threshold_seconds: u64,
    safe_folder_path: String,
    notifications_enabled: bool,
    start_at_login: bool,
}

#[derive(serde::Deserialize)]
struct LegacyAppConfigWithCloseBehavior {
    version: u32,
    watch_targets: Vec<WatchTarget>,
    protected_patterns: Vec<String>,
    default_ttl_seconds: u64,
    stale_threshold_seconds: u64,
    decaying_threshold_seconds: u64,
    safe_folder_path: String,
    notifications_enabled: bool,
    start_at_login: bool,
    close_behavior: CloseBehavior,
}

fn deserialize_config(bytes: &[u8]) -> Result<AppConfig, AppError> {
    match bincode::deserialize::<AppConfig>(bytes) {
        Ok(config) => Ok(config),
        Err(current_error) => match bincode::deserialize::<LegacyAppConfigWithCloseBehavior>(bytes)
        {
            Ok(config) => Ok(AppConfig {
                version: config.version,
                watch_targets: config.watch_targets,
                protected_patterns: config.protected_patterns,
                default_ttl_seconds: config.default_ttl_seconds,
                stale_threshold_seconds: config.stale_threshold_seconds,
                decaying_threshold_seconds: config.decaying_threshold_seconds,
                safe_folder_path: config.safe_folder_path,
                notifications_enabled: config.notifications_enabled,
                start_at_login: config.start_at_login,
                close_behavior: config.close_behavior,
                dropzone_enabled: false,
            }),
            Err(_) => match bincode::deserialize::<LegacyAppConfig>(bytes) {
                Ok(config) => Ok(AppConfig {
                    version: config.version,
                    watch_targets: config.watch_targets,
                    protected_patterns: config.protected_patterns,
                    default_ttl_seconds: config.default_ttl_seconds,
                    stale_threshold_seconds: config.stale_threshold_seconds,
                    decaying_threshold_seconds: config.decaying_threshold_seconds,
                    safe_folder_path: config.safe_folder_path,
                    notifications_enabled: config.notifications_enabled,
                    start_at_login: config.start_at_login,
                    close_behavior: CloseBehavior::Ask,
                    dropzone_enabled: false,
                }),
                Err(_) => Err(current_error.into()),
            },
        },
    }
}
