use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use redb::Database;
use uuid::Uuid;

use crate::engine::freshness::{now_seconds, tracked_file_from_metadata};
use crate::engine::paths::root_contains;
use crate::models::{
    AppConfig, AppError, AuditActionKind, AuditEntry, AutomationRule, Expiry, FileDecayState,
    RuleAction, RuleMatchExplanation, RuleMode, TrackedFile, UndoStatus, UserTriageAction,
};
use crate::rules::protected_pattern_match;
use crate::storage;
use std::sync::OnceLock;

static TRASH_SUPPORTED: OnceLock<bool> = OnceLock::new();

pub fn init_trash_support() {
    let supported = check_trash_support();
    let _ = TRASH_SUPPORTED.set(supported);
}

pub fn is_trash_supported() -> bool {
    *TRASH_SUPPORTED.get_or_init(check_trash_support)
}

fn check_trash_support() -> bool {
    let temp_dir = std::env::temp_dir();
    let file_name = format!("shelflife-trash-test-{}.txt", Uuid::new_v4());
    let temp_file = temp_dir.join(&file_name);

    if fs::write(&temp_file, "trash test").is_err() {
        return false;
    }

    let canonical_temp_file = temp_file
        .canonicalize()
        .unwrap_or_else(|_| temp_file.clone());

    if trash::delete(&temp_file).is_err() {
        let _ = fs::remove_file(&temp_file);
        return false;
    }

    let items = match trash::os_limited::list() {
        Ok(items) => items,
        Err(_) => return false,
    };

    let mut matched_item = None;
    for item in items {
        if item.name == OsStr::new(&file_name) {
            matched_item = Some(item);
            break;
        }
    }

    if let Some(item) = matched_item {
        if trash::os_limited::restore_all(std::iter::once(item)).is_ok() {
            let _ = fs::remove_file(&canonical_temp_file);
            true
        } else {
            false
        }
    } else {
        false
    }
}

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
            validate_move_destination_folder(&safe_folder, &config)?;
            fs::create_dir_all(&safe_folder)?;
            let destination = move_destination(&source, &safe_folder, None)?;
            fs::rename(&source, &destination)?;
            destination_path = Some(destination.to_string_lossy().to_string());
            apply_tracked_destination(&mut tracked, &destination);
            action_kind = AuditActionKind::Move;
            undo_status = UndoStatus::Available;
        }
        UserTriageAction::Move { destination_folder } => {
            validate_not_protected_for_filesystem_change(&source, &config)?;
            let destination_folder = PathBuf::from(destination_folder);
            validate_move_destination_folder(&destination_folder, &config)?;
            fs::create_dir_all(&destination_folder)?;
            let destination = move_destination(&source, &destination_folder, None)?;
            fs::rename(&source, &destination)?;
            destination_path = Some(destination.to_string_lossy().to_string());
            apply_tracked_destination(&mut tracked, &destination);
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
            undo_status = if is_trash_supported() {
                UndoStatus::Available
            } else {
                UndoStatus::Unavailable {
                    reason: String::from(
                        "Recycle Bin is not supported or not fully functional on this system.",
                    ),
                }
            };
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

pub fn execute_automation_rule_action(
    db: &Database,
    path: &str,
    rule: &AutomationRule,
    explanation: RuleMatchExplanation,
) -> Result<AuditEntry, AppError> {
    if !matches!(rule.mode, RuleMode::Automatic) {
        return Err(AppError::new(
            "RULE_NOT_AUTOMATIC",
            "Only automatic rules can execute without user confirmation.",
            true,
        ));
    }

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

    match &rule.action {
        RuleAction::Ignore => {
            tracked.state = FileDecayState::Ignored;
            action_kind = AuditActionKind::Ignore;
            undo_status = UndoStatus::Available;
        }
        RuleAction::Move {
            destination_folder,
            rename_template,
        } => {
            validate_not_protected_for_filesystem_change(&source, &config)?;
            let destination_folder = PathBuf::from(destination_folder);
            validate_move_destination_folder(&destination_folder, &config)?;
            fs::create_dir_all(&destination_folder)?;
            let destination =
                move_destination(&source, &destination_folder, rename_template.as_deref())?;
            fs::rename(&source, &destination)?;
            destination_path = Some(destination.to_string_lossy().to_string());
            apply_tracked_destination(&mut tracked, &destination);
            action_kind = AuditActionKind::Move;
            undo_status = UndoStatus::Available;
        }
        RuleAction::Trash => {
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
            undo_status = if is_trash_supported() {
                UndoStatus::Available
            } else {
                UndoStatus::Unavailable {
                    reason: String::from(
                        "Recycle Bin is not supported or not fully functional on this system.",
                    ),
                }
            };
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
        rule_id: Some(rule.id.clone()),
        rule_name: Some(rule.name.clone()),
        explanation: Some(explanation),
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
        AuditActionKind::Move => undo_move_like(db, &entry),
        AuditActionKind::Pin | AuditActionKind::Snooze | AuditActionKind::Ignore => {
            undo_state_only(db, &entry)
        }
        AuditActionKind::Trash => undo_trash(db, &entry),
        AuditActionKind::RulePreview => Err(AppError::new(
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
    validate_move_source_for_undo(&from, &config)?;
    let to_parent = to.parent().ok_or_else(|| {
        AppError::new(
            "UNDO_FAILED",
            "Original path has no parent folder. No file was changed.",
            true,
        )
    })?;
    validate_restore_destination_scope(to_parent, &config)?;

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

fn undo_trash(db: &Database, entry: &AuditEntry) -> Result<(), AppError> {
    let source_path = Path::new(&entry.source_path);
    let parent = source_path.parent().unwrap_or_else(|| Path::new(""));
    let canonical_parent = parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_path_buf());
    let file_name = source_path.file_name().unwrap_or_default();
    let target_original_path = canonical_parent.join(file_name);

    let items = trash::os_limited::list().map_err(|error| {
        AppError::with_details(
            "UNDO_FAILED",
            "Could not read the Recycle Bin.",
            true,
            error.to_string(),
        )
    })?;

    let mut best_match = None;
    let mut closest_time_diff = i64::MAX;

    for item in items {
        let item_parent = &item.original_parent;
        let canonical_item_parent = item_parent
            .canonicalize()
            .unwrap_or_else(|_| item_parent.to_path_buf());
        let normalized_item_path = canonical_item_parent.join(&item.name);

        let paths_match = if cfg!(target_os = "windows") {
            let path_a = normalized_item_path.to_string_lossy().to_string();
            let path_b = target_original_path.to_string_lossy().to_string();
            path_a
                .replace('\\', "/")
                .eq_ignore_ascii_case(&path_b.replace('\\', "/"))
        } else {
            normalized_item_path == target_original_path
        };

        if paths_match {
            let time_diff = (item.time_deleted - entry.timestamp as i64).abs();
            if time_diff < closest_time_diff {
                closest_time_diff = time_diff;
                best_match = Some(item);
            }
        }
    }

    let matched_item = best_match.ok_or_else(|| {
        AppError::new(
            "UNDO_FAILED",
            "The deleted file could not be found in the Recycle Bin.",
            true,
        )
    })?;

    trash::os_limited::restore_all(std::iter::once(matched_item)).map_err(|error| {
        if let trash::Error::RestoreCollision { path, .. } = &error {
            AppError::with_details(
                "UNDO_FAILED",
                "A file already exists at the restore location.",
                true,
                path.to_string_lossy().to_string(),
            )
        } else {
            AppError::with_details(
                "UNDO_FAILED",
                "Failed to restore the file from the Recycle Bin.",
                true,
                error.to_string(),
            )
        }
    })?;

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
    Ok(tracked_file_from_metadata(
        path,
        &metadata,
        None,
        config,
        config.default_ttl_seconds,
        "",
    ))
}

fn validate_source_scope(path: &Path, config: &AppConfig) -> Result<(), AppError> {
    let path = path.canonicalize()?;
    if config
        .watch_targets
        .iter()
        .filter(|target| target.enabled)
        .any(|target| root_contains(&target.path, &path))
        || root_contains(&config.safe_folder_path, &path)
    {
        return Ok(());
    }

    Err(AppError::path_out_of_scope(path.to_string_lossy().as_ref()))
}

fn validate_restore_destination_scope(parent: &Path, config: &AppConfig) -> Result<(), AppError> {
    if config
        .watch_targets
        .iter()
        .filter(|target| target.enabled)
        .any(|target| root_contains(&target.path, parent))
    {
        return Ok(());
    }

    Err(AppError::path_out_of_scope(
        parent.to_string_lossy().as_ref(),
    ))
}

fn validate_move_source_for_undo(path: &Path, config: &AppConfig) -> Result<(), AppError> {
    if path.exists() && destination_inside_watch_targets(path, config) {
        return Err(AppError::path_out_of_scope(path.to_string_lossy().as_ref()));
    }

    Ok(())
}

pub fn validate_move_destination_folder(folder: &Path, config: &AppConfig) -> Result<(), AppError> {
    if folder.as_os_str().is_empty() {
        return Err(AppError::new(
            "RULE_INVALID_DESTINATION",
            "Move destination folder is required. No file was changed.",
            true,
        ));
    }

    if destination_inside_watch_targets(folder, config) {
        return Err(AppError::with_details(
            "RULE_INVALID_DESTINATION",
            "Move destination folder must be outside all enabled watch targets.",
            true,
            folder.to_string_lossy().to_string(),
        ));
    }

    Ok(())
}

fn destination_inside_watch_targets(path: &Path, config: &AppConfig) -> bool {
    config
        .watch_targets
        .iter()
        .filter(|target| target.enabled)
        .any(|target| root_contains(&target.path, path))
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

fn apply_tracked_destination(tracked: &mut TrackedFile, destination: &Path) {
    tracked.path = destination.to_string_lossy().to_string();
    tracked.file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(&tracked.file_name)
        .to_string();
}

pub fn move_destination(
    source: &Path,
    destination_folder: &Path,
    rename_template: Option<&str>,
) -> Result<PathBuf, AppError> {
    let file_name = match rename_template {
        Some(template) if !template.trim().is_empty() => render_rename_template(source, template)?,
        _ => source
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                AppError::new(
                    "ACTION_FAILED",
                    "The file name could not be read. No file was changed.",
                    true,
                )
            })?
            .to_string(),
    };

    Ok(unique_destination(&destination_folder.join(file_name)))
}

pub fn render_rename_template(source: &Path, template: &str) -> Result<String, AppError> {
    validate_rename_template(template)?;
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
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(current_name);
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let date = Local::now().format("%Y-%m-%d").to_string();

    let rendered = template
        .replace("{name}", stem)
        .replace("{ext}", extension)
        .replace("{file}", current_name)
        .replace("{date}", &date);

    let cleaned = clean_file_name(if rendered.trim().is_empty() {
        current_name
    } else {
        &rendered
    });
    validate_windows_reserved_name(&cleaned)?;
    Ok(cleaned)
}

pub fn validate_rename_template(template: &str) -> Result<(), AppError> {
    if template.trim().is_empty() {
        return Ok(());
    }

    let invalid_character = template.chars().find(|character| {
        matches!(
            character,
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
        )
    });
    if let Some(character) = invalid_character {
        return Err(AppError::with_details(
            "RULE_INVALID_RENAME_TEMPLATE",
            "Rename template contains a character that is not valid in Windows file names.",
            true,
            character.to_string(),
        ));
    }

    let mut remaining = template;
    while let Some(open_index) = remaining.find('{') {
        let after_open = &remaining[open_index + 1..];
        let Some(close_index) = after_open.find('}') else {
            return Err(AppError::new(
                "RULE_INVALID_RENAME_TEMPLATE",
                "Rename template has an unclosed placeholder.",
                true,
            ));
        };
        let placeholder = &after_open[..close_index];
        if !matches!(placeholder, "name" | "ext" | "file" | "date") {
            return Err(AppError::with_details(
                "RULE_INVALID_RENAME_TEMPLATE",
                "Rename template contains an unknown placeholder.",
                true,
                format!("{{{placeholder}}}"),
            ));
        }
        remaining = &after_open[close_index + 1..];
    }

    if remaining.contains('}') {
        return Err(AppError::new(
            "RULE_INVALID_RENAME_TEMPLATE",
            "Rename template has a closing placeholder brace without an opening brace.",
            true,
        ));
    }

    if !template.contains('{') {
        validate_windows_reserved_name(&clean_file_name(template))?;
    }

    Ok(())
}

fn validate_windows_reserved_name(file_name: &str) -> Result<(), AppError> {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(file_name)
        .trim_end_matches('.');
    let upper = stem.to_ascii_uppercase();
    let reserved = matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if reserved {
        return Err(AppError::with_details(
            "RULE_INVALID_RENAME_TEMPLATE",
            "Rename template resolves to a reserved Windows file name.",
            true,
            file_name.to_string(),
        ));
    }

    Ok(())
}

fn clean_file_name(file_name: &str) -> String {
    let mut name = file_name
        .replace(" (1)", "")
        .replace("_copy", "")
        .replace(" copy", "");
    name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    name = name.trim_matches(' ').trim_end_matches('.').to_string();
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
    use std::path::Path;

    use crate::models::{AuditActionKind, FileDecayState, UndoStatus, UserTriageAction};
    use crate::storage;
    use crate::storage::test_util::{path_string, Fixture};

    use super::{execute_triage_action, render_rename_template};

    #[test]
    fn manual_move_preserves_name_avoids_collision_and_updates_tracked_path() {
        let fixture = Fixture::new("shelflife-test");
        let source = fixture.write_watch_file("report.txt", "download");
        let destination_folder = fixture.outside.join("sorted");
        std::fs::create_dir_all(&destination_folder).expect("destination folder should exist");
        fixture.write_file(&destination_folder.join("report.txt"), "existing");
        fixture.save_config();

        let entry = execute_triage_action(
            &fixture.db,
            &path_string(&source),
            UserTriageAction::Move {
                destination_folder: path_string(&destination_folder),
            },
        )
        .expect("move should succeed");

        let destination = entry
            .destination_path
            .expect("destination should be recorded");
        assert!(Path::new(&destination).exists());
        assert!(destination.ends_with("report-1.txt"));
        assert_eq!(entry.action_kind, AuditActionKind::Move);
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
        let fixture = Fixture::new("shelflife-test");
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
        let fixture = Fixture::new("shelflife-test");
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
        let fixture = Fixture::new("shelflife-test");
        let source = fixture.write_watch_file("report.txt", "download");
        let destination_folder = fixture.outside.join("sorted");
        std::fs::create_dir_all(&destination_folder).expect("destination folder should exist");
        let existing_destination =
            fixture.write_file(&destination_folder.join("report.txt"), "existing");
        fixture.save_config();

        let entry = execute_triage_action(
            &fixture.db,
            &path_string(&source),
            UserTriageAction::Move {
                destination_folder: path_string(&destination_folder),
            },
        )
        .expect("custom move should succeed");

        let destination = entry
            .destination_path
            .expect("destination should be recorded");
        assert!(destination.ends_with("report-1.txt"));
        assert!(Path::new(&destination).exists());
        assert!(!source.exists());
        assert!(existing_destination.exists());
    }

    #[test]
    fn scoped_custom_move_can_create_allowed_subfolder() {
        let fixture = Fixture::new("shelflife-test");
        let source = fixture.write_watch_file("report.txt", "download");
        fixture.save_config();
        let destination_folder = fixture.outside.join("sorted");
        let destination = destination_folder.join("report.txt");

        let entry = execute_triage_action(
            &fixture.db,
            &path_string(&source),
            UserTriageAction::Move {
                destination_folder: path_string(&destination_folder),
            },
        )
        .expect("custom move into outside subfolder should succeed");

        assert_eq!(entry.destination_path, Some(path_string(&destination)));
        assert!(destination.exists());
        assert!(!source.exists());
    }

    #[test]
    fn custom_move_inside_watch_target_is_rejected_before_change() {
        let fixture = Fixture::new("shelflife-test");
        let source = fixture.write_watch_file("report.txt", "download");
        fixture.save_config();
        let destination_folder = fixture.watch.join("sorted");

        let error = execute_triage_action(
            &fixture.db,
            &path_string(&source),
            UserTriageAction::Move {
                destination_folder: path_string(&destination_folder),
            },
        )
        .expect_err("in-watch destination should fail");

        assert_eq!(error.code, "RULE_INVALID_DESTINATION");
        assert!(source.exists());
        assert!(!destination_folder.exists());
    }

    #[test]
    fn protected_pattern_blocks_filesystem_changing_action() {
        let fixture = Fixture::new("shelflife-test");
        let source = fixture.write_watch_file("tax_receipt.txt", "download");
        fixture.save_config_with_protected_patterns(vec![String::from("(?i)(tax|receipt)")]);

        let error = execute_triage_action(
            &fixture.db,
            &path_string(&source),
            UserTriageAction::Move {
                destination_folder: path_string(&fixture.outside),
            },
        )
        .expect_err("protected file should not be moved");

        assert_eq!(error.code, "ACTION_FAILED");
        assert!(source.exists());
        assert!(!fixture.outside.join("tax_receipt.txt").exists());
    }

    #[test]
    fn rename_template_drops_empty_extension_trailing_dot() {
        let fixture = Fixture::new("shelflife-test");
        let source = fixture.write_watch_file("README", "download");

        let rendered =
            render_rename_template(&source, "{name}.{ext}").expect("template should render");

        assert_eq!(rendered, "README");
    }

    #[test]
    fn rename_template_rejects_unknown_placeholder() {
        let fixture = Fixture::new("shelflife-test");
        let source = fixture.write_watch_file("report.pdf", "download");

        let error = render_rename_template(&source, "{month}-{file}")
            .expect_err("unknown placeholder should fail");

        assert_eq!(error.code, "RULE_INVALID_RENAME_TEMPLATE");
    }

    #[test]
    fn protected_pattern_still_allows_pin() {
        let fixture = Fixture::new("shelflife-test");
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
        let fixture = Fixture::new("shelflife-test");
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
        let fixture = Fixture::new("shelflife-test");
        let source = fixture.write_outside_file("outside.txt", "private");
        fixture.save_config();

        let error =
            execute_triage_action(&fixture.db, &path_string(&source), UserTriageAction::Ignore)
                .expect_err("out-of-scope action should fail");

        assert_eq!(error.code, "PATH_OUT_OF_SCOPE");
        assert!(source.exists());
    }
}
