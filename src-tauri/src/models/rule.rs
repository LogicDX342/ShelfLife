use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RuleMode {
    PreviewOnly,
    AskFirst,
    Automatic,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RuleAction {
    Trash,
    Move {
        destination_folder: String,
        rename_template: Option<String>,
    },
    Ignore,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SizeCondition {
    Any,
    LessThan(u64),
    GreaterThan(u64),
    Between { min: u64, max: u64 },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RuleConditions {
    pub extensions: Vec<String>,
    pub filename_globs: Vec<String>,
    pub filename_regexes: Vec<String>,
    pub source_domains: Vec<String>,
    pub size: SizeCondition,
}

impl Default for RuleConditions {
    fn default() -> Self {
        Self {
            extensions: Vec::new(),
            filename_globs: Vec::new(),
            filename_regexes: Vec::new(),
            source_domains: Vec::new(),
            size: SizeCondition::Any,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AutomationRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    pub watch_path: String,
    pub ttl_seconds: u64,
    pub conditions: RuleConditions,
    pub action: RuleAction,
    pub mode: RuleMode,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RuleMatchExplanation {
    pub file_path: String,
    pub size_bytes: Option<u64>,
    pub rule_id: Option<String>,
    pub rule_name: Option<String>,
    pub matched_extension: bool,
    pub matched_size: bool,
    pub matched_origin: Option<String>,
    pub matched_filename_pattern: Option<String>,
    pub proposed_action: Option<RuleAction>,
    pub mode: Option<RuleMode>,
    pub message: String,
}
