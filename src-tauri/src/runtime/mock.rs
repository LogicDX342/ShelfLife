use redb::Database;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{App, Manager};

use crate::models::{
    AppConfig, AuditActionKind, AuditEntry, AutomationRule, Expiry, FileDecayState, OriginEvidence,
    RuleAction, RuleConditions, RuleMode, SizeCondition, TrackedFile, UndoStatus, WatchTarget,
};
use crate::storage;

pub fn is_mock_mode() -> bool {
    if std::env::var("SHELFLIFE_MOCK").is_ok() || std::env::var("VITE_MOCK").is_ok() {
        return true;
    }

    if cfg!(debug_assertions) {
        for path in &[Path::new(".env.mock"), Path::new("../.env.mock")] {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    if content.contains("VITE_MOCK=true") || content.contains("SHELFLIFE_MOCK=true")
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn set_file_times(path: &Path, time: SystemTime) -> Result<(), std::io::Error> {
    let file = std::fs::File::options().write(true).open(path)?;
    let times = std::fs::FileTimes::new()
        .set_accessed(time)
        .set_modified(time);
    file.set_times(times)?;
    Ok(())
}

pub fn preload_mock_data(app: &App, db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    let mut mock_watch_dir = PathBuf::from("E:\\shelflife-mock-watch");

    // Attempt to create E:\shelflife-mock-watch. Fall back to AppData if E: drive does not exist/is not accessible.
    if std::fs::create_dir_all(&mock_watch_dir).is_err() {
        mock_watch_dir = app.path().app_data_dir()?.join("shelflife-mock-watch");
        if mock_watch_dir.exists() {
            let _ = std::fs::remove_dir_all(&mock_watch_dir);
        }
        std::fs::create_dir_all(&mock_watch_dir)?;
    } else {
        let _ = std::fs::remove_dir_all(&mock_watch_dir);
        std::fs::create_dir_all(&mock_watch_dir)?;
    }

    let now = SystemTime::now();
    let now_secs = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

    // Define file paths with folder levels
    let file_todo = mock_watch_dir.join("todo_list.txt");

    let file_exe = mock_watch_dir
        .join("Installers")
        .join("chrome_installer.exe");
    std::fs::create_dir_all(file_exe.parent().unwrap())?;

    let file_pdf = mock_watch_dir
        .join("Documents")
        .join("2025")
        .join("annual_report_2025.pdf");
    std::fs::create_dir_all(file_pdf.parent().unwrap())?;

    let file_log = mock_watch_dir.join("Logs").join("temporary_log.log");
    std::fs::create_dir_all(file_log.parent().unwrap())?;

    let file_zip = mock_watch_dir.join("Archives").join("huge_dataset.zip");
    std::fs::create_dir_all(file_zip.parent().unwrap())?;

    let file_jpg = mock_watch_dir.join("Photos").join("vacation_photo.jpg");
    std::fs::create_dir_all(file_jpg.parent().unwrap())?;

    let file_pinned = mock_watch_dir.join("Notes").join("important_notes.txt");
    std::fs::create_dir_all(file_pinned.parent().unwrap())?;

    // 1. Write mock files to disk with specific modification times in the past
    // a. Fresh file: todo_list.txt (modified now)
    std::fs::write(
        &file_todo,
        "1. Refactor Svelte components\n2. Add mock mode\n",
    )?;

    // b. Decaying installer: chrome_installer.exe (modified 2 days ago)
    std::fs::write(&file_exe, "EXE dummy content".repeat(10000))?; // ~170 KB
    let time_exe = now - Duration::from_secs(2 * 24 * 60 * 60);
    set_file_times(&file_exe, time_exe)?;

    // c. Stale report: annual_report_2025.pdf (modified 10 days ago)
    std::fs::write(&file_pdf, "PDF dummy content".repeat(100))?; // ~1.6 KB
    let time_pdf = now - Duration::from_secs(10 * 24 * 60 * 60);
    set_file_times(&file_pdf, time_pdf)?;

    // d. Ignored log: temporary_log.log (modified 12 days ago)
    std::fs::write(&file_log, "DEBUG: App started\nINFO: Log initialized\n")?;
    let time_log = now - Duration::from_secs(12 * 24 * 60 * 60);
    set_file_times(&file_log, time_log)?;

    // e. Decaying large archive: huge_dataset.zip (modified 6 days ago)
    std::fs::write(&file_zip, "ZIP dummy content".repeat(20000))?; // ~340 KB
    let time_zip = now - Duration::from_secs(6 * 24 * 60 * 60);
    set_file_times(&file_zip, time_zip)?;

    // f. Stale photo: vacation_photo.jpg (modified 20 days ago)
    std::fs::write(&file_jpg, "JPG dummy content".repeat(50))?;
    let time_jpg = now - Duration::from_secs(20 * 24 * 60 * 60);
    set_file_times(&file_jpg, time_jpg)?;

    // g. Pinned file: important_notes.txt (modified now, but database will mark as Permanent)
    std::fs::write(
        &file_pinned,
        "This is an important pinned file that will not decay.",
    )?;

    // 2. Set Config in database
    let mock_target_id = String::from("mock-watch");
    let safe_folder = app.path().app_data_dir()?.join("shelflife-mock-safe");
    let safe_folder_str = safe_folder.to_string_lossy().to_string();
    let config = AppConfig {
        watch_targets: vec![WatchTarget {
            id: mock_target_id.clone(),
            path: mock_watch_dir.to_string_lossy().to_string(),
            enabled: true,
            recursive: true,
            ignore_patterns: Vec::new(),
            include_hidden_patterns: Vec::new(),
        }],
        default_ttl_seconds: 30 * 24 * 60 * 60,    // 30 days
        stale_threshold_seconds: 5 * 24 * 60 * 60, // 5 days
        decaying_threshold_seconds: 24 * 60 * 60,  // 1 day
        safe_folder_path: safe_folder_str.clone(),
        notifications_enabled: true,
        start_at_login: false,
        close_behavior: crate::models::CloseBehavior::Ask,
        dropzone_enabled: true,
    };
    storage::save_config(db, &config)?;

    // Make sure safe folder exists too
    let _ = std::fs::create_dir_all(&safe_folder);

    // 3. Save Automation Rules
    // Rule 1: Clean Installer Executables
    let rule_exe = AutomationRule {
        id: String::from("clean-exe-rule"),
        name: String::from("Clean Installer Executables"),
        enabled: true,
        priority: 10,
        watch_path: mock_watch_dir.to_string_lossy().to_string(),
        ttl_seconds: 3 * 24 * 60 * 60, // 3 days
        conditions: RuleConditions {
            extensions: vec![String::from("exe")],
            ..RuleConditions::default()
        },
        action: RuleAction::Trash,
        mode: RuleMode::AskFirst,
        created_at: now_secs,
        updated_at: now_secs,
    };
    storage::rules::save_rule(db, &rule_exe)?;

    // Rule 2: Archive Large Datasets (> 100 KB)
    let rule_zip = AutomationRule {
        id: String::from("archive-zip-rule"),
        name: String::from("Archive Large Datasets"),
        enabled: true,
        priority: 20,
        watch_path: mock_watch_dir.to_string_lossy().to_string(),
        ttl_seconds: 5 * 24 * 60 * 60, // 5 days
        conditions: RuleConditions {
            extensions: vec![String::from("zip"), String::from("tar.gz")],
            size: SizeCondition::GreaterThan(100 * 1024), // > 100 KB
            ..RuleConditions::default()
        },
        action: RuleAction::Move {
            destination_folder: safe_folder_str.clone(),
            rename_template: None,
        },
        mode: RuleMode::Automatic,
        created_at: now_secs,
        updated_at: now_secs,
    };
    storage::rules::save_rule(db, &rule_zip)?;

    // Rule 3: Temporary Log Files
    let rule_log = AutomationRule {
        id: String::from("ignore-log-rule"),
        name: String::from("Ignore Log Files"),
        enabled: true,
        priority: 5,
        watch_path: mock_watch_dir.to_string_lossy().to_string(),
        ttl_seconds: 30 * 24 * 60 * 60,
        conditions: RuleConditions {
            extensions: vec![String::from("log")],
            ..RuleConditions::default()
        },
        action: RuleAction::Ignore,
        mode: RuleMode::Automatic,
        created_at: now_secs,
        updated_at: now_secs,
    };
    storage::rules::save_rule(db, &rule_log)?;

    #[allow(clippy::too_many_arguments)]
    fn preseed_tracked_file(
        db: &Database,
        path: &Path,
        name: &str,
        target_id: &str,
        size: u64,
        time_past_secs: u64,
        expiry: Expiry,
        state: FileDecayState,
        matched_rule_ids: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let file = TrackedFile {
            path: path.to_string_lossy().to_string(),
            file_name: String::from(name),
            watch_target_id: String::from(target_id),
            size_bytes: size,
            first_seen_at: time_past_secs,
            last_observed_mtime: Some(time_past_secs),
            last_observed_atime: Some(time_past_secs),
            last_user_action_at: None,
            freshness_at: time_past_secs,
            expiry,
            state,
            matched_rule_ids,
            origin: OriginEvidence::Unknown,
        };
        storage::tracked::upsert_tracked_file(db, &file)?;
        Ok(())
    }

    // 4. Pre-seed the files in tracked files database
    let time_exe_secs = now_secs - 2 * 24 * 60 * 60;
    preseed_tracked_file(
        db,
        &file_exe,
        "chrome_installer.exe",
        &mock_target_id,
        170000,
        time_exe_secs,
        Expiry::At(time_exe_secs + 3 * 24 * 60 * 60),
        FileDecayState::Decaying,
        vec![String::from("clean-exe-rule")],
    )?;

    let time_pdf_secs = now_secs - 10 * 24 * 60 * 60;
    preseed_tracked_file(
        db,
        &file_pdf,
        "annual_report_2025.pdf",
        &mock_target_id,
        1600,
        time_pdf_secs,
        Expiry::At(time_pdf_secs + 30 * 24 * 60 * 60),
        FileDecayState::Stale,
        vec![],
    )?;

    let time_log_secs = now_secs - 12 * 24 * 60 * 60;
    preseed_tracked_file(
        db,
        &file_log,
        "temporary_log.log",
        &mock_target_id,
        50,
        time_log_secs,
        Expiry::At(time_log_secs + 30 * 24 * 60 * 60),
        FileDecayState::Ignored,
        vec![String::from("ignore-log-rule")],
    )?;

    let time_zip_secs = now_secs - 6 * 24 * 60 * 60;
    preseed_tracked_file(
        db,
        &file_zip,
        "huge_dataset.zip",
        &mock_target_id,
        340000,
        time_zip_secs,
        Expiry::At(time_zip_secs + 5 * 24 * 60 * 60),
        FileDecayState::Decaying,
        vec![String::from("archive-zip-rule")],
    )?;

    let time_jpg_secs = now_secs - 20 * 24 * 60 * 60;
    preseed_tracked_file(
        db,
        &file_jpg,
        "vacation_photo.jpg",
        &mock_target_id,
        1000,
        time_jpg_secs,
        Expiry::At(time_jpg_secs + 30 * 24 * 60 * 60),
        FileDecayState::Stale,
        vec![],
    )?;

    let pinned_file = TrackedFile {
        path: file_pinned.to_string_lossy().to_string(),
        file_name: String::from("important_notes.txt"),
        watch_target_id: mock_target_id.clone(),
        size_bytes: 52,
        first_seen_at: now_secs,
        last_observed_mtime: Some(now_secs),
        last_observed_atime: Some(now_secs),
        last_user_action_at: Some(now_secs),
        freshness_at: now_secs,
        expiry: Expiry::Permanent,
        state: FileDecayState::Pinned,
        matched_rule_ids: Vec::new(),
        origin: OriginEvidence::WindowsZoneIdentifier {
            zone_id: Some(3), // Zone 3 is Internet
            host_url: Some(String::from("https://github.com")),
            referrer_url: None,
        },
    };
    storage::tracked::upsert_tracked_file(db, &pinned_file)?;

    // 5. Save Mock Audit Log Entries
    // Entry 1: Trashed old log
    let seq1 = storage::audit::next_audit_sequence(db)?;
    let audit1 = AuditEntry {
        id: uuid::Uuid::new_v4().to_string(),
        sequence: seq1,
        timestamp: now_secs - 5 * 24 * 60 * 60,
        action_kind: AuditActionKind::Trash,
        source_path: format!("{}\\old_debug.log", mock_watch_dir.to_string_lossy()),
        destination_path: None,
        file_name: String::from("old_debug.log"),
        size_bytes: 12000,
        rule_id: Some(String::from("ignore-log-rule")),
        rule_name: Some(String::from("Ignore Log Files")),
        explanation: None,
        undo_status: UndoStatus::Unavailable {
            reason: String::from("File was permanently deleted from recycle bin"),
        },
    };
    storage::audit::append_audit_entry(db, &audit1)?;

    // Entry 2: Archived zip backup
    let seq2 = storage::audit::next_audit_sequence(db)?;
    let dest_path = format!("{}\\backup_2026_06_15.zip", safe_folder_str);
    let audit2 = AuditEntry {
        id: uuid::Uuid::new_v4().to_string(),
        sequence: seq2,
        timestamp: now_secs - 3 * 24 * 60 * 60,
        action_kind: AuditActionKind::Move,
        source_path: format!(
            "{}\\backup_2026_06_15.zip",
            mock_watch_dir.to_string_lossy()
        ),
        destination_path: Some(dest_path),
        file_name: String::from("backup_2026_06_15.zip"),
        size_bytes: 150000000,
        rule_id: Some(String::from("archive-zip-rule")),
        rule_name: Some(String::from("Archive Large Datasets")),
        explanation: None,
        undo_status: UndoStatus::Available,
    };
    storage::audit::append_audit_entry(db, &audit2)?;

    // Entry 3: Manually Pinned notes.txt
    let seq3 = storage::audit::next_audit_sequence(db)?;
    let audit3 = AuditEntry {
        id: uuid::Uuid::new_v4().to_string(),
        sequence: seq3,
        timestamp: now_secs - 24 * 60 * 60,
        action_kind: AuditActionKind::Pin,
        source_path: file_pinned.to_string_lossy().to_string(),
        destination_path: None,
        file_name: String::from("important_notes.txt"),
        size_bytes: 52,
        rule_id: None,
        rule_name: None,
        explanation: None,
        undo_status: UndoStatus::Completed,
    };
    storage::audit::append_audit_entry(db, &audit3)?;

    println!(
        "Mock database successfully preloaded with watch targets, rules, files, and audit entries."
    );
    Ok(())
}

pub fn cleanup_mock_data(app: &tauri::AppHandle) {
    let mock_watch_dir_e = PathBuf::from("E:\\shelflife-mock-watch");
    if mock_watch_dir_e.exists() {
        let _ = std::fs::remove_dir_all(&mock_watch_dir_e);
    }

    if let Ok(app_data) = app.path().app_data_dir() {
        let mock_watch_dir_appdata = app_data.join("shelflife-mock-watch");
        if mock_watch_dir_appdata.exists() {
            let _ = std::fs::remove_dir_all(&mock_watch_dir_appdata);
        }
    }

    println!("Mock watch folders successfully removed on exit.");
}
