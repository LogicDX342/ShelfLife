use serde::{Deserialize, Serialize};

use super::OriginEvidence;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Expiry {
    At(u64),
    Permanent,
    SnoozedUntil(u64),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum FileDecayState {
    Fresh,
    Stale,
    Decaying,
    Pinned,
    Ignored,
    Missing,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TrackedFile {
    pub path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub first_seen_at: u64,
    pub last_observed_mtime: Option<u64>,
    pub last_observed_atime: Option<u64>,
    pub last_user_action_at: Option<u64>,
    pub freshness_at: u64,
    pub expiry: Expiry,
    pub state: FileDecayState,
    pub matched_rule_ids: Vec<String>,
    pub origin: OriginEvidence,
}
