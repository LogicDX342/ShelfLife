use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Expiry {
    At(u64),
    Permanent,
    SnoozedUntil(u64),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDecayState {
    Fresh,
    Stale,
    Decaying,
    Pinned,
    ManuallyIgnored,
    RuleIgnored,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TrackedFile {
    pub path: String,
    pub file_name: String,
    pub watch_target_id: String,
    pub size_bytes: u64,
    pub last_observed_mtime: Option<u64>,
    pub freshness_at: u64,
    pub expiry: Expiry,
    pub state: FileDecayState,
    pub matched_rule_ids: Vec<String>,
    pub origin_url: Option<String>,
}

impl TrackedFile {
    /// Returns `true` when any tracked field differs from `previous`.
    pub fn changed_from(&self, previous: &TrackedFile) -> bool {
        self.file_name != previous.file_name
            || self.size_bytes != previous.size_bytes
            || self.last_observed_mtime != previous.last_observed_mtime
            || self.freshness_at != previous.freshness_at
            || self.expiry != previous.expiry
            || self.state != previous.state
            || self.matched_rule_ids != previous.matched_rule_ids
            || self.origin_url != previous.origin_url
    }
}
