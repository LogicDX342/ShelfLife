use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum CloseBehavior {
    Ask,
    HideToTray,
    Quit,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct WatchTarget {
    pub id: String,
    pub path: String,
    pub enabled: bool,
    pub recursive: bool,
    pub default_ttl_seconds: Option<u64>,
    pub ignore_patterns: Vec<String>,
    /// Glob patterns for hidden directories that should be included in recursive scanning.
    /// By default all hidden directories are skipped. Add patterns like `.git` or `.vscode`
    /// to allow scanning into specific hidden subdirectories.
    #[serde(default)]
    pub include_hidden_patterns: Vec<String>,
    pub rule_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub version: u32,
    pub watch_targets: Vec<WatchTarget>,
    pub protected_patterns: Vec<String>,
    pub default_ttl_seconds: u64,
    pub stale_threshold_seconds: u64,
    pub decaying_threshold_seconds: u64,
    pub safe_folder_path: String,
    pub notifications_enabled: bool,
    pub start_at_login: bool,
    pub close_behavior: CloseBehavior,
    pub dropzone_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            watch_targets: Vec::new(),
            protected_patterns: vec![String::from(
                "(?i)(tax|invoice|receipt|passport|lease|contract|project_alpha)",
            )],
            default_ttl_seconds: 30 * 24 * 60 * 60,
            stale_threshold_seconds: 5 * 24 * 60 * 60,
            decaying_threshold_seconds: 24 * 60 * 60,
            safe_folder_path: default_safe_folder(),
            notifications_enabled: true,
            start_at_login: false,
            close_behavior: CloseBehavior::Ask,
            dropzone_enabled: false,
        }
    }
}

fn default_safe_folder() -> String {
    std::env::var("USERPROFILE")
        .map(|home| format!("{home}\\shelflife-safe"))
        .unwrap_or_else(|_| String::from("shelflife-safe"))
}
