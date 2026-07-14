use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::engine::freshness::tracked_file_from_metadata;
use crate::models::{
    AppConfig, AutomationRule, RuleAction, RuleConditions, RuleMode, SizeCondition, WatchTarget,
};
use crate::storage;

pub struct Fixture {
    pub root: PathBuf,
    pub watch: PathBuf,
    pub outside: PathBuf,
    pub safe: PathBuf,
    pub db: storage::Database,
}

impl Fixture {
    pub fn new(prefix: &str) -> Self {
        let root = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
        let watch = root.join("watch");
        let outside = root.join("outside");
        let safe = root.join("safe");
        fs::create_dir_all(&watch).expect("watch directory should be created");
        fs::create_dir_all(&outside).expect("outside directory should be created");
        fs::create_dir_all(&safe).expect("safe directory should be created");
        let db = storage::open_database(root.join("test.sqlite")).expect("database should open");
        Self {
            root,
            watch,
            outside,
            safe,
            db,
        }
    }

    pub fn config(&self) -> AppConfig {
        AppConfig {
            watch_targets: vec![self.watch_target(false)],
            safe_folder_path: path_string(&self.safe),
            ..AppConfig::default()
        }
    }

    pub fn watch_target(&self, recursive: bool) -> WatchTarget {
        WatchTarget {
            id: String::from("watch"),
            path: path_string(&self.watch),
            enabled: true,
            recursive,
            ignore_patterns: Vec::new(),
        }
    }

    pub fn rule(&self) -> AutomationRule {
        AutomationRule {
            id: String::from("zip-rule"),
            name: String::from("Zip downloads"),
            enabled: true,
            priority: 10,
            watch_path: path_string(&self.watch),
            ttl_seconds: 86_400,
            conditions: RuleConditions {
                extensions: vec![String::from("zip")],
                filename_globs: Vec::new(),
                filename_regexes: Vec::new(),
                source_domains: Vec::new(),
                size: SizeCondition::Any,
            },
            action: RuleAction::Trash,
            mode: RuleMode::PreviewOnly,
            created_at: 1,
            updated_at: 1,
        }
    }

    pub fn save_config(&self) {
        storage::save_config(&self.db, &self.config()).expect("config should save");
    }

    pub fn save_config_without_watch_targets(&self) {
        self.save_config_with_targets(Vec::new());
    }

    pub fn save_config_with_targets(&self, watch_targets: Vec<WatchTarget>) {
        let config = AppConfig {
            watch_targets,
            safe_folder_path: path_string(&self.safe),
            ..AppConfig::default()
        };
        storage::save_config(&self.db, &config).expect("config should save");
    }

    pub fn write_watch_file(&self, name: &str, content: &str) -> PathBuf {
        self.write_file(&self.watch.join(name), content)
    }

    pub fn write_outside_file(&self, name: &str, content: &str) -> PathBuf {
        self.write_file(&self.outside.join(name), content)
    }

    pub fn write_file(&self, path: &Path, content: &str) -> PathBuf {
        fs::write(path, content).expect("test file should be written");
        path.to_path_buf()
    }

    pub fn track_file(&self, path: &Path) {
        let metadata = fs::metadata(path).expect("metadata should exist");
        let config = AppConfig::default();
        let mut tracked = tracked_file_from_metadata(path, &metadata, None, &config, "");
        tracked.origin_url = None;
        storage::tracked::upsert_tracked_file(&self.db, &tracked)
            .expect("tracked file should save");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
