use serde::{Deserialize, Serialize};

use super::RuleMatchExplanation;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AuditActionKind {
    Trash,
    Move,
    Pin,
    Snooze,
    Ignore,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum UndoStatus {
    Available,
    Unavailable { reason: String },
    Completed,
    Failed { reason: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    pub id: String,
    pub sequence: u64,
    pub timestamp: u64,
    pub action_kind: AuditActionKind,
    pub source_path: String,
    pub destination_path: Option<String>,
    pub file_name: String,
    pub size_bytes: u64,
    pub rule_id: Option<String>,
    pub rule_name: Option<String>,
    pub explanation: Option<RuleMatchExplanation>,
    pub undo_status: UndoStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct BulkTriageFailure {
    pub path: String,
    pub error: super::AppError,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct BulkTriageResult {
    pub entries: Vec<AuditEntry>,
    pub failures: Vec<BulkTriageFailure>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum UserTriageAction {
    Pin,
    Snooze { seconds: u64 },
    Ignore,
    Move { destination_folder: String },
    TrashNow,
}
