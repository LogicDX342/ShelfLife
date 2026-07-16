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
    pub ignore_patterns: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub watch_targets: Vec<WatchTarget>,
    pub default_ttl_seconds: u64,
    pub stale_threshold_seconds: u64,
    pub decaying_threshold_seconds: u64,
    pub default_move_destination: Option<String>,
    pub notifications_enabled: bool,
    pub start_at_login: bool,
    pub close_behavior: CloseBehavior,
    pub dropzone_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            watch_targets: Vec::new(),
            default_ttl_seconds: 30 * 24 * 60 * 60,
            stale_threshold_seconds: 5 * 24 * 60 * 60,
            decaying_threshold_seconds: 24 * 60 * 60,
            default_move_destination: None,
            notifications_enabled: true,
            start_at_login: false,
            close_behavior: CloseBehavior::Ask,
            dropzone_enabled: false,
        }
    }
}
