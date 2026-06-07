use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use redb::Database;
use uuid::Uuid;

use crate::engine::freshness::{now_seconds, tracked_file_from_metadata};
use crate::models::{
    AppConfig, AppError, AuditActionKind, AuditEntry, Expiry, FileDecayState, TrackedFile,
    UndoStatus, UserTriageAction,
};
use crate::rules::protected_pattern_match;
use crate::storage;

pub fn execute_triage_action(
    db: &Database,
    path: &str,
    action: UserTriageAction,
) -> Result<AuditEntry, AppError> {
    let config = storage::get_config(db)?;
    let source = PathBuf::from(path);
    if !source.exists() {
        return Err(AppError::path_not_found(path));
    }
    validate_source_scope(&source, &config)?;

    let mut tracked = load_or_create_tracked(db, &source, &config)?;
    let original_tracked_path = tracked.path.clone();
    let timestamp = now_seconds();
    let mut destination_path = None;
    let action_kind;
    let undo_status;

    match action {
        UserTriageAction::Pin => {
            tracked.state = FileDecayState::Pinned;
            tracked.expiry = Expiry::Permanent;
            tracked.last_user_action_at = Some(timestamp);
            action_kind = AuditActionKind::Pin;
            undo_status = UndoStatus::Available;
        }
        UserTriageAction::Snooze { seconds } => {
            let until = timestamp + seconds;
            tracked.freshness_at = until;
            tracked.expiry = Expiry::SnoozedUntil(until);
            tracked.state = FileDecayState::Fresh;
            tracked.last_user_action_at = Some(timestamp);
            action_kind = AuditActionKind::Snooze;
            undo_status = UndoStatus::Available;
        }
        UserTriageAction::Ignore => {
            tracked.state = FileDecayState::Ignored;
            tracked.last_user_action_at = Some(timestamp);
            action_kind = AuditActionKind::Ignore;
            undo_status = UndoStatus::Available;
        }
        UserTriageAction::MoveToSafeFolder => {
            validate_not_protected_for_filesystem_change(&source, &config)?;
            let safe_folder = PathBuf::from(&config.safe_folder_path);
            fs::create_dir_all(&safe_folder)?;
            let destination = unique_destination(&safe_folder.join(&tracked.file_name));
            fs::rename(&source, &destination)?;
            destination_path = Some(destination.to_string_lossy().to_string());
            tracked.path = destination.to_string_lossy().to_string();
            action_kind = AuditActionKind::Move;
            undo_status = UndoStatus::Available;
        }
        UserTriageAction::Move {
            destination_path: destination,
        } => {
            validate_not_protected_for_filesystem_change(&source, &config)?;
            let destination = PathBuf::from(destination);
            let parent = destination.parent().ok_or_else(|| {
                AppError::new(
                    "RULE_INVALID_DESTINATION",
                    "Move destination has no parent directory. No file was changed.",
                    true,
                )
            })?;
            validate_destination_scope(parent, &config)?;
            fs::create_dir_all(parent)?;
            let destination = unique_destination(&destination);
            fs::rename(&source, &destination)?;
            destination_path = Some(destination.to_string_lossy().to_string());
            tracked.path = destination.to_string_lossy().to_string();
            action_kind = AuditActionKind::Move;
            undo_status = UndoStatus::Available;
        }
        UserTriageAction::TrashNow => {
            validate_not_protected_for_filesystem_change(&source, &config)?;
            trash::delete(&source).map_err(|error| {
                AppError::with_details(
                    "ACTION_FAILED",
                    "The file could not be moved to the Recycle Bin. No raw deletion was attempted.",
                    true,
                    error.to_string(),
                )
            })?;
            tracked.state = FileDecayState::Missing;
            action_kind = AuditActionKind::Trash;
            undo_status = UndoStatus::Unavailable {
                reason: String::from("Recycle Bin restore location is not exposed reliably."),
            };
        }
        UserTriageAction::Rename { template } => {
            validate_not_protected_for_filesystem_change(&source, &config)?;
            let destination = rename_destination(&source, &template)?;
            fs::rename(&source, &destination)?;
            destination_path = Some(destination.to_string_lossy().to_string());
            tracked.path = destination.to_string_lossy().to_string();
            tracked.file_name = destination
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(&tracked.file_name)
                .to_string();
            action_kind = AuditActionKind::Rename;
            undo_status = UndoStatus::Available;
        }
    }

    tracked.last_user_action_at = Some(timestamp);
    if tracked.path != original_tracked_path {
        storage::tracked::remove_tracked_file(db, &original_tracked_path)?;
    }
    storage::tracked::upsert_tracked_file(db, &tracked)?;

    let entry = AuditEntry {
        id: Uuid::new_v4().to_string(),
        sequence: storage::audit::next_audit_sequence(db)?,
        timestamp,
        action_kind,
        source_path: path.to_string(),
        destination_path,
        file_name: tracked.file_name.clone(),
        size_bytes: tracked.size_bytes,
        rule_id: None,
        rule_name: None,
        explanation: None,
        undo_status,
    };
    storage::audit::append_audit_entry(db, &entry)?;
    Ok(entry)
}

pub fn undo_audit_entry(db: &Database, audit_id: &str) -> Result<AuditEntry, AppError> {
    let mut entry = storage::audit::get_audit_entry_by_id(db, audit_id)?
        .ok_or_else(|| AppError::new("UNDO_UNAVAILABLE", "Audit entry was not found.", true))?;

    match entry.undo_status {
        UndoStatus::Available => {}
        _ => {
            return Err(AppError::new(
                "UNDO_UNAVAILABLE",
                "This action cannot currently be undone.",
                true,
            ))
        }
    }

    let result = match entry.action_kind {
        AuditActionKind::Move | AuditActionKind::Rename => undo_move_like(db, &entry),
        AuditActionKind::Pin | AuditActionKind::Snooze | AuditActionKind::Ignore => {
            undo_state_only(db, &entry)
        }
        AuditActionKind::Trash | AuditActionKind::RulePreview => Err(AppError::new(
            "UNDO_UNAVAILABLE",
            "This audit entry has no reliable filesystem undo path.",
            true,
        )),
    };

    match result {
        Ok(()) => entry.undo_status = UndoStatus::Completed,
        Err(error) => {
            entry.undo_status = UndoStatus::Failed {
                reason: error.message.clone(),
            };
            storage::audit::update_audit_entry(db, &entry)?;
            return Err(error);
        }
    }

    storage::audit::update_audit_entry(db, &entry)?;
    Ok(entry)
}

fn undo_move_like(db: &Database, entry: &AuditEntry) -> Result<(), AppError> {
    let config = storage::get_config(db)?;
    let destination = entry.destination_path.as_ref().ok_or_else(|| {
        AppError::new(
            "UNDO_UNAVAILABLE",
            "The audit entry did not record a destination path.",
            true,
        )
    })?;
    let from = PathBuf::from(destination);
    let to = PathBuf::from(&entry.source_path);
    validate_source_scope(&from, &config)?;
    let to_parent = to.parent().ok_or_else(|| {
        AppError::new(
            "UNDO_FAILED",
            "Original path has no parent folder. No file was changed.",
            true,
        )
    })?;
    validate_destination_scope(to_parent, &config)?;

    if !from.exists() {
        return Err(AppError::new(
            "UNDO_FAILED",
            "The moved file is no longer at its recorded destination.",
            true,
        ));
    }
    if to.exists() {
        return Err(AppError::new(
            "UNDO_FAILED",
            "A file already exists at the original path.",
            true,
        ));
    }

    fs::rename(&from, &to)?;
    if let Some(mut tracked) = storage::tracked::get_tracked_file(db, destination)? {
        tracked.path = entry.source_path.clone();
        tracked.file_name = to
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(&entry.file_name)
            .to_string();
        storage::tracked::remove_tracked_file(db, destination)?;
        storage::tracked::upsert_tracked_file(db, &tracked)?;
    }
    Ok(())
}

fn undo_state_only(db: &Database, entry: &AuditEntry) -> Result<(), AppError> {
    if let Some(mut tracked) = storage::tracked::get_tracked_file(db, &entry.source_path)? {
        tracked.state = FileDecayState::Fresh;
        tracked.expiry =
            Expiry::At(tracked.freshness_at + storage::get_config(db)?.default_ttl_seconds);
        storage::tracked::upsert_tracked_file(db, &tracked)?;
    }
    Ok(())
}

fn load_or_create_tracked(
    db: &Database,
    path: &Path,
    config: &AppConfig,
) -> Result<TrackedFile, AppError> {
    let path_string = path.to_string_lossy().to_string();
    if let Some(file) = storage::tracked::get_tracked_file(db, &path_string)? {
        return Ok(file);
    }

    let metadata = fs::metadata(path)?;
    Ok(tracked_file_from_metadata(path, &metadata, None, config))
}

fn validate_source_scope(path: &Path, config: &AppConfig) -> Result<(), AppError> {
    let path = path.canonicalize()?;
    if config
        .watch_targets
        .iter()
        .filter(|target| target.enabled)
        .any(|target| canonical_root_contains(&target.path, &path))
        || canonical_root_contains(&config.safe_folder_path, &path)
    {
        return Ok(());
    }

    Err(AppError::path_out_of_scope(path.to_string_lossy().as_ref()))
}

fn validate_destination_scope(parent: &Path, config: &AppConfig) -> Result<(), AppError> {
    if config
        .watch_targets
        .iter()
        .filter(|target| target.enabled)
        .any(|target| configured_root_contains(&target.path, parent))
        || configured_root_contains(&config.safe_folder_path, parent)
    {
        return Ok(());
    }

    Err(AppError::path_out_of_scope(
        parent.to_string_lossy().as_ref(),
    ))
}

fn validate_not_protected_for_filesystem_change(
    path: &Path,
    config: &AppConfig,
) -> Result<(), AppError> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if let Some(pattern) = protected_pattern_match(file_name, &config.protected_patterns)? {
        return Err(AppError::with_details(
            "ACTION_FAILED",
            "Protected file was not changed. Adjust protected patterns before moving, renaming, or trashing this file.",
            true,
            pattern,
        ));
    }

    Ok(())
}

fn canonical_root_contains(root: &str, path: &Path) -> bool {
    let Some(root) = normalize_configured_path(Path::new(root)) else {
        return false;
    };
    let Some(path) = normalize_configured_path(path) else {
        return false;
    };
    path.starts_with(root)
}

fn configured_root_contains(root: &str, path: &Path) -> bool {
    canonical_root_contains(root, path)
}

fn normalize_configured_path(path: &Path) -> Option<PathBuf> {
    let mut suffix = Vec::new();
    let mut cursor = path.to_path_buf();

    loop {
        if let Ok(canonical) = cursor.canonicalize() {
            let mut normalized = canonical;
            for component in suffix.iter().rev() {
                normalized.push(component);
            }
            return Some(normalized);
        }

        let component = cursor.file_name()?.to_os_string();
        if component == OsStr::new(".") || component == OsStr::new("..") {
            return None;
        }
        suffix.push(component);
        cursor = cursor.parent()?.to_path_buf();
    }
}

fn rename_destination(source: &Path, template: &str) -> Result<PathBuf, AppError> {
    let parent = source.parent().ok_or_else(|| {
        AppError::new(
            "ACTION_FAILED",
            "The file has no parent directory for rename. No file was changed.",
            true,
        )
    })?;
    let current_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            AppError::new(
                "ACTION_FAILED",
                "The file name could not be read. No file was changed.",
                true,
            )
        })?;

    let cleaned = clean_file_name(if template.trim().is_empty() {
        current_name
    } else {
        template
    });
    Ok(unique_destination(&parent.join(cleaned)))
}

fn clean_file_name(file_name: &str) -> String {
    let mut name = file_name
        .replace(" (1)", "")
        .replace("_copy", "")
        .replace(" copy", "");
    name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        String::from("renamed-file")
    } else {
        name
    }
}

fn unique_destination(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }

    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let extension = path.extension().and_then(|value| value.to_str());

    for index in 1.. {
        let file_name = match extension {
            Some(extension) => format!("{stem}-{index}.{extension}"),
            None => format!("{stem}-{index}"),
        };
        let candidate = parent.join(file_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("unbounded collision search should always return")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use uuid::Uuid;

    use crate::models::{
        AppConfig, AuditActionKind, FileDecayState, UndoStatus, UserTriageAction, WatchTarget,
    };
    use crate::storage;

    use super::execute_triage_action;

    #[test]
    fn rename_avoids_collision_and_updates_tracked_path() {
        let fixture = Fixture::new();
        let source = fixture.write_watch_file("report (1).txt", "download");
        fixture.write_watch_file("report.txt", "existing");
        fixture.save_config();

        let entry = execute_triage_action(
            &fixture.db,
            &path_string(&source),
            UserTriageAction::Rename {
                template: String::new(),
            },
        )
        .expect("rename should succeed");

        let destination = entry
            .destination_path
            .expect("destination should be recorded");
        assert!(Path::new(&destination).exists());
        assert!(destination.ends_with("report-1.txt"));
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &path_string(&source))
                .expect("tracked lookup should work")
                .is_none()
        );
        assert!(
            storage::tracked::get_tracked_file(&fixture.db, &destination)
                .expect("tracked lookup should work")
                .is_some()
        );
    }

    #[test]
    fn move_to_safe_folder_audits_and_undo_restores_file() {
        let fixture = Fixture::new();
        let source = fixture.write_watch_file("notes.txt", "download");
        fixture.save_config();

        let entry = execute_triage_action(
            &fixture.db,
            &path_string(&source),
            UserTriageAction::MoveToSafeFolder,
        )
        .expect("move should succeed");

        assert_eq!(entry.action_kind, AuditActionKind::Move);
        assert!(matches!(entry.undo_status, UndoStatus::Available));
        assert!(!source.exists());
        assert!(Path::new(entry.destination_path.as_ref().unwrap()).exists());

        let undone = super::undo_audit_entry(&fixture.db, &entry.id).expect("undo should succeed");
        assert!(matches!(undone.undo_status, UndoStatus::Completed));
        assert!(source.exists());
    }

    #[test]
    fn undo_move_revalidates_original_path_scope() {
        let fixture = Fixture::new();
        let source = fixture.write_watch_file("notes.txt", "download");
        fixture.save_config();
        let entry = execute_triage_action(
            &fixture.db,
            &path_string(&source),
            UserTriageAction::MoveToSafeFolder,
        )
        .expect("move should succeed");
        fixture.save_config_without_watch_targets();

        let error = super::undo_audit_entry(&fixture.db, &entry.id)
            .expect_err("undo should fail when original path is out of scope");

        assert_eq!(error.code, "PATH_OUT_OF_SCOPE");
        assert!(!source.exists());
        assert!(Path::new(entry.destination_path.as_ref().unwrap()).exists());
    }

    #[test]
    fn scoped_custom_move_succeeds_and_avoids_destination_collision() {
        let fixture = Fixture::new();
        let source = fixture.write_watch_file("report.txt", "download");
        let existing_destination = fixture.write_watch_file("sorted.txt", "existing");
        fixture.save_config();

        let entry = execute_triage_action(
            &fixture.db,
            &path_string(&source),
            UserTriageAction::Move {
                destination_path: path_string(&existing_destination),
            },
        )
        .expect("custom move should succeed");

        let destination = entry
            .destination_path
            .expect("destination should be recorded");
        assert!(destination.ends_with("sorted-1.txt"));
        assert!(Path::new(&destination).exists());
        assert!(!source.exists());
        assert!(existing_destination.exists());
    }

    #[test]
    fn scoped_custom_move_can_create_allowed_subfolder() {
        let fixture = Fixture::new();
        let source = fixture.write_watch_file("report.txt", "download");
        fixture.save_config();
        let destination = fixture.watch.join("sorted").join("report.txt");

        let entry = execute_triage_action(
            &fixture.db,
            &path_string(&source),
            UserTriageAction::Move {
                destination_path: path_string(&destination),
            },
        )
        .expect("custom move into allowed subfolder should succeed");

        assert_eq!(entry.destination_path, Some(path_string(&destination)));
        assert!(destination.exists());
        assert!(!source.exists());
    }

    #[test]
    fn custom_move_outside_scope_is_rejected_before_change() {
        let fixture = Fixture::new();
        let source = fixture.write_watch_file("report.txt", "download");
        fixture.save_config();
        let destination = fixture.outside.join("report.txt");

        let error = execute_triage_action(
            &fixture.db,
            &path_string(&source),
            UserTriageAction::Move {
                destination_path: path_string(&destination),
            },
        )
        .expect_err("out-of-scope destination should fail");

        assert_eq!(error.code, "PATH_OUT_OF_SCOPE");
        assert!(source.exists());
        assert!(!destination.exists());
    }

    #[test]
    fn protected_pattern_blocks_filesystem_changing_action() {
        let fixture = Fixture::new();
        let source = fixture.write_watch_file("tax_receipt.txt", "download");
        fixture.save_config_with_protected_patterns(vec![String::from("(?i)(tax|receipt)")]);

        let error = execute_triage_action(
            &fixture.db,
            &path_string(&source),
            UserTriageAction::Rename {
                template: String::from("renamed.txt"),
            },
        )
        .expect_err("protected file should not be renamed");

        assert_eq!(error.code, "ACTION_FAILED");
        assert!(source.exists());
        assert!(!fixture.watch.join("renamed.txt").exists());
    }

    #[test]
    fn protected_pattern_still_allows_pin() {
        let fixture = Fixture::new();
        let source = fixture.write_watch_file("tax_receipt.txt", "download");
        fixture.save_config_with_protected_patterns(vec![String::from("(?i)(tax|receipt)")]);

        let entry =
            execute_triage_action(&fixture.db, &path_string(&source), UserTriageAction::Pin)
                .expect("pin should remain available for protected files");

        assert_eq!(entry.action_kind, AuditActionKind::Pin);
        assert!(source.exists());
    }

    #[test]
    fn ignore_action_creates_audit_row_and_marks_file_ignored() {
        let fixture = Fixture::new();
        let source = fixture.write_watch_file("scratch.tmpx", "download");
        fixture.save_config();

        let entry =
            execute_triage_action(&fixture.db, &path_string(&source), UserTriageAction::Ignore)
                .expect("ignore should succeed");
        let tracked = storage::tracked::get_tracked_file(&fixture.db, &path_string(&source))
            .expect("tracked lookup should work")
            .expect("tracked file should exist");

        assert_eq!(entry.action_kind, AuditActionKind::Ignore);
        assert_eq!(tracked.state, FileDecayState::Ignored);
        assert_eq!(
            storage::audit::list_audit_entries(&fixture.db)
                .expect("audit list should work")
                .len(),
            1
        );
    }

    #[test]
    fn out_of_scope_action_is_rejected_before_change() {
        let fixture = Fixture::new();
        let source = fixture.write_outside_file("outside.txt", "private");
        fixture.save_config();

        let error =
            execute_triage_action(&fixture.db, &path_string(&source), UserTriageAction::Ignore)
                .expect_err("out-of-scope action should fail");

        assert_eq!(error.code, "PATH_OUT_OF_SCOPE");
        assert!(source.exists());
    }

    struct Fixture {
        root: PathBuf,
        watch: PathBuf,
        outside: PathBuf,
        safe: PathBuf,
        db: std::sync::Arc<redb::Database>,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("shelflife-test-{}", Uuid::new_v4()));
            let watch = root.join("watch");
            let outside = root.join("outside");
            let safe = root.join("safe");
            fs::create_dir_all(&watch).expect("watch directory should be created");
            fs::create_dir_all(&outside).expect("outside directory should be created");
            fs::create_dir_all(&safe).expect("safe directory should be created");
            let db = storage::open_database(root.join("test.redb")).expect("database should open");
            Self {
                root,
                watch,
                outside,
                safe,
                db,
            }
        }

        fn save_config(&self) {
            self.save_config_with_protected_patterns(Vec::new());
        }

        fn save_config_with_protected_patterns(&self, protected_patterns: Vec<String>) {
            self.save_config_with_targets_and_patterns(
                vec![WatchTarget {
                    id: String::from("watch"),
                    path: path_string(&self.watch),
                    enabled: true,
                    recursive: false,
                    default_ttl_seconds: None,
                    ignore_patterns: Vec::new(),
                    include_hidden_patterns: Vec::new(),
                    rule_ids: Vec::new(),
                }],
                protected_patterns,
            );
        }

        fn save_config_without_watch_targets(&self) {
            self.save_config_with_targets_and_patterns(Vec::new(), Vec::new());
        }

        fn save_config_with_targets_and_patterns(
            &self,
            watch_targets: Vec<WatchTarget>,
            protected_patterns: Vec<String>,
        ) {
            let config = AppConfig {
                watch_targets,
                safe_folder_path: path_string(&self.safe),
                protected_patterns,
                ..AppConfig::default()
            };
            storage::save_config(&self.db, &config).expect("config should save");
        }

        fn write_watch_file(&self, name: &str, content: &str) -> PathBuf {
            self.write_file(&self.watch.join(name), content)
        }

        fn write_outside_file(&self, name: &str, content: &str) -> PathBuf {
            self.write_file(&self.outside.join(name), content)
        }

        fn write_file(&self, path: &Path, content: &str) -> PathBuf {
            fs::write(path, content).expect("test file should be written");
            path.to_path_buf()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn path_string(path: &Path) -> String {
        path.to_string_lossy().to_string()
    }
}
