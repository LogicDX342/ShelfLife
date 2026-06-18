use serde::{Deserialize, Serialize};

use super::{AppError, AuditEntry, RuleAction, RuleMatchExplanation, RuleMode, WatchTarget};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DropzoneFile {
    pub path: String,
    pub file_name: String,
    pub size_bytes: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DropzoneRejectedFile {
    pub path: String,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DropzoneRuleGroup {
    pub rule_id: String,
    pub rule_name: String,
    pub mode: RuleMode,
    pub action: RuleAction,
    pub file_paths: Vec<String>,
    pub file_count: usize,
    pub total_size_bytes: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DropzonePreview {
    pub files: Vec<DropzoneFile>,
    pub rejected_files: Vec<DropzoneRejectedFile>,
    pub watch_targets: Vec<WatchTarget>,
    pub rule_groups: Vec<DropzoneRuleGroup>,
    pub preview_only: Vec<RuleMatchExplanation>,
    pub unmatched_files: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DropzoneActionFailure {
    pub path: String,
    pub error: AppError,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DropzoneActionResult {
    pub entries: Vec<AuditEntry>,
    pub failures: Vec<DropzoneActionFailure>,
}
