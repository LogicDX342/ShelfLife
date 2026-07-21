use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{App, Manager};

use crate::models::{
    AppConfig, AuditActionKind, AuditEntry, AutomationRule, Expiry, FileDecayState, RuleAction,
    RuleConditions, RuleMode, SizeCondition, TrackedFile, UndoStatus, WatchTarget,
};
use crate::storage;
use crate::storage::Database;

const MOCK_ROOT_DIR: &str = "mock-mode";
const MOCK_DB_FILE: &str = "shelflife.sqlite";
const MOCK_WATCH_DIR: &str = "watch";
const MOCK_MOVE_DESTINATION_DIR: &str = "sorted";
const MOCK_TARGET_ID: &str = "mock-watch";
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

#[derive(Clone, Copy)]
enum MockExpiry {
    AfterDays(u64),
    Permanent,
    SnoozedForDays(u64),
}

type MockTrackedSpec = (
    MockExpiry,
    FileDecayState,
    &'static [&'static str],
    Option<&'static str>,
);
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
    ("Projects/todo_list.txt", "1. Refactor Svelte components\n2. Add mock mode\n", 1, 0, None),
    ("Documents/meeting_notes.md", "# Meeting notes\n- Confirm launch checklist\n- Review retention rules\n", 1, 1, None),
    ("Downloads/chrome_installer.exe", "EXE dummy content", 10_000, 3, Some((MockExpiry::AfterDays(3), FileDecayState::Decaying, &["clean-exe-rule"], Some("https://dl.google.com/")))),
    ("Downloads/database_tools.msi", "MSI dummy content", 8_000, 0, Some((MockExpiry::AfterDays(3), FileDecayState::Fresh, &["clean-exe-rule"], Some("https://downloads.example.com/")))),
    ("Documents/annual_report_2025.pdf", "PDF dummy content", 100, 10, Some((MockExpiry::AfterDays(30), FileDecayState::Stale, &[], Some("https://contoso.sharepoint.com/")))),
    ("Documents/vendor_contract.docx", "DOCX dummy content", 1_000, 3, Some((MockExpiry::AfterDays(30), FileDecayState::Fresh, &[], None))),
    ("Documents/quarterly_budget.xlsx", "XLSX dummy content", 8_000, 7, Some((MockExpiry::AfterDays(30), FileDecayState::Stale, &[], None))),
    ("Documents/product_launch.pptx", "PPTX dummy content", 12_000, 29, Some((MockExpiry::AfterDays(30), FileDecayState::Decaying, &[], None))),
    ("Logs/temporary_log.log", "DEBUG: App started\nINFO: Log initialized\n", 20, 12, Some((MockExpiry::AfterDays(30), FileDecayState::RuleIgnored, &["ignore-log-rule"], None))),
    ("Logs/build_output.log", "INFO: Compiling ShelfLife\nWARN: Mock warning\n", 250, 1, Some((MockExpiry::AfterDays(30), FileDecayState::RuleIgnored, &["ignore-log-rule"], None))),
    ("Downloads/huge_dataset.zip", "ZIP dummy content", 20_000, 4, Some((MockExpiry::AfterDays(5), FileDecayState::Stale, &["archive-zip-rule"], Some("https://github.com/")))),
    ("Downloads/source_bundle.7z", "7Z dummy content", 12_000, 4, Some((MockExpiry::AfterDays(5), FileDecayState::Stale, &["archive-zip-rule"], None))),
    ("Media/vacation_photo.jpg", "JPG dummy content", 5_000, 20, Some((MockExpiry::AfterDays(30), FileDecayState::Stale, &[], Some("https://photos.example.com/")))),
    ("Media/logo_concept.svg", "<svg><rect width=\"64\" height=\"64\" /></svg>\n", 10, 2, Some((MockExpiry::AfterDays(30), FileDecayState::Fresh, &[], None))),
    ("Media/checkout_error.png", "PNG dummy content", 9_000, 8, Some((MockExpiry::AfterDays(7), FileDecayState::Decaying, &["organize-screenshots-rule"], None))),
    ("Media/product_demo.mp4", "MP4 dummy content", 80_000, 8, Some((MockExpiry::AfterDays(30), FileDecayState::Stale, &["review-media-rule"], None))),
    ("Media/interview_recording.mp3", "MP3 dummy content", 25_000, 2, Some((MockExpiry::AfterDays(30), FileDecayState::Fresh, &[], None))),
    ("Downloads/customer_export.csv", "id,name,status\n1,Ada,active\n2,Grace,active\n", 2_000, 15, Some((MockExpiry::AfterDays(30), FileDecayState::Stale, &[], None))),
    ("Downloads/api_response.json", "{\"status\":\"ok\",\"items\":[1,2,3]}\n", 100, 0, Some((MockExpiry::AfterDays(30), FileDecayState::Fresh, &[], None))),
    ("Projects/app.ts", "export const mockMode = true;\n", 40, 6, Some((MockExpiry::AfterDays(30), FileDecayState::Stale, &[], None))),
    ("Projects/engine.rs", "pub fn reconcile() {}\n", 40, 29, Some((MockExpiry::AfterDays(30), FileDecayState::Decaying, &[], None))),
    ("Documents/important_notes.txt", "This is an important pinned file that will not decay.", 1, 0, Some((MockExpiry::Permanent, FileDecayState::Pinned, &[], Some("https://github.com/")))),
    ("Documents/reference_links.md", "# Reference links\n- https://tauri.app/\n- https://svelte.dev/\n", 1, 14, Some((MockExpiry::AfterDays(30), FileDecayState::ManuallyIgnored, &[], None))),
    ("Documents/conference_receipt.pdf", "PDF receipt dummy content", 500, 20, Some((MockExpiry::SnoozedForDays(7), FileDecayState::Fresh, &[], None))),
];

#[derive(Clone, Copy)]
enum MockRuleAction {
    Trash,
    Move,
    MoveAndRename(&'static str),
    Ignore,
}

type MockRuleSpec = (
    &'static str,
    &'static str,
    bool,
    i32,
    u64,
    &'static [&'static str],
    Option<u64>,
    MockRuleAction,
    RuleMode,
);

// id, name, enabled, priority, TTL days, extensions, minimum size, action, mode
#[rustfmt::skip]
const MOCK_RULES: &[MockRuleSpec] = &[
    ("archive-zip-rule", "Archive Large Datasets", true, 20, 5, &["zip", "7z"], Some(100 * 1024), MockRuleAction::Move, RuleMode::Automatic),
    ("review-media-rule", "Review Large Media", true, 15, 14, &["mp4", "mov"], Some(500 * 1024), MockRuleAction::Move, RuleMode::PreviewOnly),
    ("organize-screenshots-rule", "Organize Screenshots", true, 12, 7, &["png"], None, MockRuleAction::MoveAndRename("{date}-{name}.{ext}"), RuleMode::AskFirst),
    ("clean-exe-rule", "Clean Installer Packages", true, 10, 3, &["exe", "msi"], None, MockRuleAction::Trash, RuleMode::AskFirst),
    ("ignore-log-rule", "Ignore Log Files", true, 5, 30, &["log"], None, MockRuleAction::Ignore, RuleMode::Automatic),
    ("clean-spreadsheets-rule", "Clean Old Spreadsheets", false, 1, 90, &["csv", "xlsx"], None, MockRuleAction::Trash, RuleMode::Automatic),
];

#[derive(Clone, Copy)]
enum MockUndoStatus {
    Available,
    Completed,
    Unavailable(&'static str),
    Failed(&'static str),
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
    (21, AuditActionKind::Trash, "old_debug.log", None, 12_000, Some(("ignore-log-rule", "Ignore Log Files")), MockUndoStatus::Unavailable("File was permanently deleted from the Recycle Bin")),
    (14, AuditActionKind::Move, "backup_2026_06_15.zip", Some("Backups/backup_2026_06_15.zip"), 150_000_000, Some(("archive-zip-rule", "Archive Large Datasets")), MockUndoStatus::Available),
    (10, AuditActionKind::Ignore, "Documents/reference_links.md", None, 62, None, MockUndoStatus::Available),
    (7, AuditActionKind::Snooze, "Documents/conference_receipt.pdf", None, 12_000, None, MockUndoStatus::Available),
    (5, AuditActionKind::Pin, "Documents/important_notes.txt", None, 52, None, MockUndoStatus::Completed),
    (3, AuditActionKind::Trash, "crash_dump.dmp", None, 8_400_000, None, MockUndoStatus::Failed("The previous Recycle Bin lookup did not complete")),
    (1, AuditActionKind::Move, "presentation_draft.pptx", Some("Presentations/presentation_draft.pptx"), 216_000, None, MockUndoStatus::Unavailable("The destination file was changed after the move")),
];

pub struct MockWorkspace {
    pub db_path: PathBuf,
    watch_dir: PathBuf,
    move_destination_dir: PathBuf,
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
    let move_destination_dir = root.join(MOCK_MOVE_DESTINATION_DIR);
    std::fs::create_dir_all(&watch_dir)?;
    std::fs::create_dir_all(&move_destination_dir)?;

    Ok(MockWorkspace {
        db_path: root.join(MOCK_DB_FILE),
        watch_dir,
        move_destination_dir,
    })
}

fn set_file_times(path: &Path, time: SystemTime) -> Result<(), std::io::Error> {
    let file = std::fs::File::options().write(true).open(path)?;
    let times = std::fs::FileTimes::new().set_modified(time);
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
    let move_destination_path = workspace
        .move_destination_dir
        .to_string_lossy()
        .into_owned();

    storage::save_config(
        db,
        &AppConfig {
            watch_targets: vec![WatchTarget {
                id: String::from(MOCK_TARGET_ID),
                path: watch_path.clone(),
                enabled: true,
                recursive: true,
                ignore_patterns: Vec::new(),
            }],
            default_ttl_seconds: 30 * SECONDS_PER_DAY,
            stale_threshold_seconds: 5 * SECONDS_PER_DAY,
            decaying_threshold_seconds: SECONDS_PER_DAY,
            default_move_destination: Some(move_destination_path.clone()),
            dropzone_enabled: true,
            ..AppConfig::default()
        },
    )?;

    for (id, name, enabled, priority, ttl_days, extensions, minimum_size, action, mode) in
        MOCK_RULES
    {
        let action = match *action {
            MockRuleAction::Trash => RuleAction::Trash,
            MockRuleAction::Move => RuleAction::Move {
                destination_folder: move_destination_path.clone(),
                rename_template: None,
            },
            MockRuleAction::MoveAndRename(template) => RuleAction::Move {
                destination_folder: move_destination_path.clone(),
                rename_template: Some(template.to_string()),
            },
            MockRuleAction::Ignore => RuleAction::Ignore,
        };
        storage::rules::save_rule(
            db,
            &AutomationRule {
                id: id.to_string(),
                name: name.to_string(),
                enabled: *enabled,
                priority: *priority,
                watch_path: watch_path.clone(),
                timing: crate::models::RuleTiming::AfterSeconds(*ttl_days * SECONDS_PER_DAY),
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

        let Some((expiry, state, matched_rule_ids, origin_url)) = tracked else {
            continue;
        };
        let metadata = std::fs::metadata(&path)?;
        let observed_at = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expiry = match *expiry {
            MockExpiry::AfterDays(days) => Expiry::At(observed_at + days * SECONDS_PER_DAY),
            MockExpiry::Permanent => Expiry::Permanent,
            MockExpiry::SnoozedForDays(days) => {
                Expiry::SnoozedUntil(now_secs + days * SECONDS_PER_DAY)
            }
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
                size_bytes: metadata.len(),
                last_observed_mtime: Some(observed_at),
                freshness_at: observed_at,
                expiry,
                state: *state,
                matched_rule_ids: matched_rule_ids.iter().map(ToString::to_string).collect(),
                origin_url: origin_url.map(ToString::to_string),
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
            MockUndoStatus::Failed(reason) => UndoStatus::Failed {
                reason: reason.to_string(),
            },
        };
        let destination_path = (*destination)
            .map(|relative_path| join_mock_path(&workspace.move_destination_dir, relative_path));
        if matches!(action_kind, AuditActionKind::Move)
            && matches!(&undo_status, UndoStatus::Available)
        {
            if let Some(path) = &destination_path {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(path, "Mock moved file awaiting undo.")?;
            }
        }
        storage::audit::upsert_audit_entry(
            db,
            &AuditEntry {
                id: uuid::Uuid::new_v4().to_string(),
                sequence: storage::audit::next_audit_sequence(db)?,
                timestamp: now_secs.saturating_sub(*age_days * SECONDS_PER_DAY),
                action_kind: action_kind.clone(),
                source_path: join_mock_path(&workspace.watch_dir, source)
                    .to_string_lossy()
                    .into_owned(),
                destination_path: destination_path.map(|path| path.to_string_lossy().into_owned()),
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
        "Mock database preloaded with {} files, {} rules, and {} audit entries.",
        MOCK_FILES.len(),
        MOCK_RULES.len(),
        MOCK_AUDIT_ENTRIES.len()
    );
    Ok(())
}
