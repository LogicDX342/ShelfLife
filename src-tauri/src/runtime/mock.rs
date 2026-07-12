use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{App, Manager};

use crate::models::{
    AppConfig, AuditActionKind, AuditEntry, AutomationRule, Expiry, FileDecayState, OriginEvidence,
    RuleAction, RuleConditions, RuleMode, SizeCondition, TrackedFile, UndoStatus, WatchTarget,
};
use crate::storage;
use crate::storage::Database;

const MOCK_ROOT_DIR: &str = "mock-mode";
const MOCK_DB_FILE: &str = "shelflife.sqlite";
const MOCK_WATCH_DIR: &str = "watch";
const MOCK_SAFE_DIR: &str = "safe";
const MOCK_TARGET_ID: &str = "mock-watch";
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

type MockTrackedSpec = (u64, Option<u64>, FileDecayState, &'static [&'static str]);
type MockFileSpec = (
    &'static str,
    &'static str,
    usize,
    u64,
    Option<MockTrackedSpec>,
);

// relative path, content chunk, repetitions, age in days, tracked metadata
#[rustfmt::skip]
const MOCK_FILES: &[MockFileSpec] = &[
    ("todo_list.txt", "1. Refactor Svelte components\n2. Add mock mode\n", 1, 0, None),
    ("Installers/chrome_installer.exe", "EXE dummy content", 10_000, 2, Some((170_000, Some(3), FileDecayState::Decaying, &["clean-exe-rule"]))),
    ("Documents/2025/annual_report_2025.pdf", "PDF dummy content", 100, 10, Some((1_600, Some(30), FileDecayState::Stale, &[]))),
    ("Logs/temporary_log.log", "DEBUG: App started\nINFO: Log initialized\n", 1, 12, Some((50, Some(30), FileDecayState::Ignored, &["ignore-log-rule"]))),
    ("Archives/huge_dataset.zip", "ZIP dummy content", 20_000, 6, Some((340_000, Some(5), FileDecayState::Decaying, &["archive-zip-rule"]))),
    ("Photos/vacation_photo.jpg", "JPG dummy content", 50, 20, Some((1_000, Some(30), FileDecayState::Stale, &[]))),
    ("Notes/important_notes.txt", "This is an important pinned file that will not decay.", 1, 0, Some((52, None, FileDecayState::Pinned, &[]))),
];

#[derive(Clone, Copy)]
enum MockRuleAction {
    Trash,
    MoveToSafeFolder,
    Ignore,
}

type MockRuleSpec = (
    &'static str,
    &'static str,
    i32,
    u64,
    &'static [&'static str],
    Option<u64>,
    MockRuleAction,
    RuleMode,
);

// id, name, priority, TTL days, extensions, minimum size, action, mode
#[rustfmt::skip]
const MOCK_RULES: &[MockRuleSpec] = &[
    ("clean-exe-rule", "Clean Installer Executables", 10, 3, &["exe"], None, MockRuleAction::Trash, RuleMode::AskFirst),
    ("archive-zip-rule", "Archive Large Datasets", 20, 5, &["zip", "tar.gz"], Some(100 * 1024), MockRuleAction::MoveToSafeFolder, RuleMode::Automatic),
    ("ignore-log-rule", "Ignore Log Files", 5, 30, &["log"], None, MockRuleAction::Ignore, RuleMode::Automatic),
];

#[derive(Clone, Copy)]
enum MockUndoStatus {
    Available,
    Completed,
    Unavailable(&'static str),
}

type MockAuditSpec = (
    u64,
    AuditActionKind,
    &'static str,
    Option<&'static str>,
    u64,
    Option<(&'static str, &'static str)>,
    MockUndoStatus,
);

// age in days, action, source, destination, size, rule identity, undo state
#[rustfmt::skip]
const MOCK_AUDIT_ENTRIES: &[MockAuditSpec] = &[
    (5, AuditActionKind::Trash, "old_debug.log", None, 12_000, Some(("ignore-log-rule", "Ignore Log Files")), MockUndoStatus::Unavailable("File was permanently deleted from recycle bin")),
    (3, AuditActionKind::Move, "backup_2026_06_15.zip", Some("backup_2026_06_15.zip"), 150_000_000, Some(("archive-zip-rule", "Archive Large Datasets")), MockUndoStatus::Available),
    (1, AuditActionKind::Pin, "Notes/important_notes.txt", None, 52, None, MockUndoStatus::Completed),
];

pub struct MockWorkspace {
    pub db_path: PathBuf,
    watch_dir: PathBuf,
    safe_dir: PathBuf,
}

pub fn is_mock_mode() -> bool {
    env_flag_enabled("SHELFLIFE_MOCK")
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "True"))
        .unwrap_or(false)
}

pub fn reset_mock_workspace(app: &App) -> Result<MockWorkspace, Box<dyn std::error::Error>> {
    let root = app.path().app_data_dir()?.join(MOCK_ROOT_DIR);
    if root.exists() {
        std::fs::remove_dir_all(&root)?;
    }

    let watch_dir = root.join(MOCK_WATCH_DIR);
    let safe_dir = root.join(MOCK_SAFE_DIR);
    std::fs::create_dir_all(&watch_dir)?;
    std::fs::create_dir_all(&safe_dir)?;

    Ok(MockWorkspace {
        db_path: root.join(MOCK_DB_FILE),
        watch_dir,
        safe_dir,
    })
}

fn set_file_times(path: &Path, time: SystemTime) -> Result<(), std::io::Error> {
    let file = std::fs::File::options().write(true).open(path)?;
    let times = std::fs::FileTimes::new()
        .set_accessed(time)
        .set_modified(time);
    file.set_times(times)
}

fn join_mock_path(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, component| path.join(component))
}

pub fn seed_mock_workspace(
    db: &Database,
    workspace: &MockWorkspace,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = SystemTime::now();
    let now_secs = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let watch_path = workspace.watch_dir.to_string_lossy().into_owned();
    let safe_path = workspace.safe_dir.to_string_lossy().into_owned();

    storage::save_config(
        db,
        &AppConfig {
            watch_targets: vec![WatchTarget {
                id: String::from(MOCK_TARGET_ID),
                path: watch_path.clone(),
                enabled: true,
                recursive: true,
                ignore_patterns: Vec::new(),
                include_hidden_patterns: Vec::new(),
            }],
            default_ttl_seconds: 30 * SECONDS_PER_DAY,
            stale_threshold_seconds: 5 * SECONDS_PER_DAY,
            decaying_threshold_seconds: SECONDS_PER_DAY,
            safe_folder_path: safe_path.clone(),
            dropzone_enabled: true,
            ..AppConfig::default()
        },
    )?;

    for (id, name, priority, ttl_days, extensions, minimum_size, action, mode) in MOCK_RULES {
        let action = match *action {
            MockRuleAction::Trash => RuleAction::Trash,
            MockRuleAction::MoveToSafeFolder => RuleAction::Move {
                destination_folder: safe_path.clone(),
                rename_template: None,
            },
            MockRuleAction::Ignore => RuleAction::Ignore,
        };
        storage::rules::save_rule(
            db,
            &AutomationRule {
                id: id.to_string(),
                name: name.to_string(),
                enabled: true,
                priority: *priority,
                watch_path: watch_path.clone(),
                ttl_seconds: *ttl_days * SECONDS_PER_DAY,
                conditions: RuleConditions {
                    extensions: extensions.iter().map(ToString::to_string).collect(),
                    size: (*minimum_size)
                        .map(SizeCondition::GreaterThan)
                        .unwrap_or(SizeCondition::Any),
                    ..RuleConditions::default()
                },
                action,
                mode: mode.clone(),
                created_at: now_secs,
                updated_at: now_secs,
            },
        )?;
    }

    for (relative_path, content, repeat, age_days, tracked) in MOCK_FILES {
        let path = join_mock_path(&workspace.watch_dir, relative_path);
        let observed_at = now_secs.saturating_sub(*age_days * SECONDS_PER_DAY);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content.repeat(*repeat))?;
        if *age_days > 0 {
            set_file_times(
                &path,
                now - Duration::from_secs(*age_days * SECONDS_PER_DAY),
            )?;
        }

        let Some((size_bytes, ttl_days, state, matched_rule_ids)) = tracked else {
            continue;
        };
        let permanent = ttl_days.is_none();
        let expiry = (*ttl_days)
            .map(|days| Expiry::At(observed_at + days * SECONDS_PER_DAY))
            .unwrap_or(Expiry::Permanent);
        let origin = if permanent {
            OriginEvidence::WindowsZoneIdentifier {
                zone_id: Some(3),
                host_url: Some(String::from("https://github.com")),
                referrer_url: None,
            }
        } else {
            OriginEvidence::Unknown
        };

        storage::tracked::upsert_tracked_file(
            db,
            &TrackedFile {
                path: path.to_string_lossy().into_owned(),
                file_name: path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or(relative_path)
                    .to_string(),
                watch_target_id: String::from(MOCK_TARGET_ID),
                size_bytes: *size_bytes,
                first_seen_at: observed_at,
                last_observed_mtime: Some(observed_at),
                last_observed_atime: Some(observed_at),
                last_user_action_at: permanent.then_some(observed_at),
                freshness_at: observed_at,
                expiry,
                state: state.clone(),
                matched_rule_ids: matched_rule_ids.iter().map(ToString::to_string).collect(),
                origin,
            },
        )?;
    }

    for (age_days, action_kind, source, destination, size_bytes, rule, undo_status) in
        MOCK_AUDIT_ENTRIES
    {
        let (rule_id, rule_name) = (*rule)
            .map(|(id, name)| (Some(id.to_string()), Some(name.to_string())))
            .unwrap_or((None, None));
        let undo_status = match *undo_status {
            MockUndoStatus::Available => UndoStatus::Available,
            MockUndoStatus::Completed => UndoStatus::Completed,
            MockUndoStatus::Unavailable(reason) => UndoStatus::Unavailable {
                reason: reason.to_string(),
            },
        };
        storage::audit::append_audit_entry(
            db,
            &AuditEntry {
                id: uuid::Uuid::new_v4().to_string(),
                sequence: storage::audit::next_audit_sequence(db)?,
                timestamp: now_secs.saturating_sub(*age_days * SECONDS_PER_DAY),
                action_kind: action_kind.clone(),
                source_path: join_mock_path(&workspace.watch_dir, source)
                    .to_string_lossy()
                    .into_owned(),
                destination_path: (*destination).map(|relative_path| {
                    join_mock_path(&workspace.safe_dir, relative_path)
                        .to_string_lossy()
                        .into_owned()
                }),
                file_name: Path::new(source)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or(source)
                    .to_string(),
                size_bytes: *size_bytes,
                rule_id,
                rule_name,
                explanation: None,
                undo_status,
            },
        )?;
    }

    println!(
        "Mock database successfully preloaded with watch targets, rules, files, and audit entries."
    );
    Ok(())
}
