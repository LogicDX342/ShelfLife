use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use uuid::Uuid;

use crate::engine::paths::PathScope;
use crate::engine::{now_seconds, tracked_file_from_metadata};
use crate::models::{
    AppConfig, AppError, AuditActionKind, AuditEntry, AutomationRule, Expiry, FileDecayState,
    RuleAction, RuleMatchExplanation, RuleMode, TrackedFile, UndoStatus, UserTriageAction,
};
use crate::storage;
use crate::storage::Database;
use std::sync::OnceLock;

const DROPZONE_INGEST_AUDIT_ID: &str = "__dropzone_ingest__";
const DROPZONE_RULE_AUDIT_ID_PREFIX: &str = "__dropzone_rule__:";

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

pub(crate) fn execute_triage_action_audited(
    db: &Database,
    path: &str,
    action: UserTriageAction,
) -> Result<AuditEntry, FileActionFailure> {
    execute_file_action(
        db,
        path,
        RequestedFileAction::User(action),
        SourcePolicy::Watched,
        ActionAuditContext::default(),
    )
}

pub(crate) fn execute_automation_rule_action(
    db: &Database,
    path: &str,
    rule: &AutomationRule,
    explanation: RuleMatchExplanation,
) -> Result<AuditEntry, FileActionFailure> {
    if !matches!(rule.mode, RuleMode::Automatic) {
        return Err(AppError::new(
            "RULE_NOT_AUTOMATIC",
            "Only automatic rules can execute without user confirmation.",
            true,
        )
        .into());
    }

    execute_file_action(
        db,
        path,
        RequestedFileAction::Rule(&rule.action),
        SourcePolicy::Watched,
        ActionAuditContext {
            rule_id: Some(rule.id.clone()),
            rule_name: Some(rule.name.clone()),
            explanation: Some(explanation),
        },
    )
}

pub(crate) fn execute_dropzone_rule_action_audited(
    db: &Database,
    path: &str,
    rule: &AutomationRule,
    explanation: RuleMatchExplanation,
) -> Result<AuditEntry, FileActionFailure> {
    if matches!(rule.mode, RuleMode::PreviewOnly) {
        return Err(AppError::new(
            "RULE_NOT_EXECUTABLE",
            "PreviewOnly rules cannot change files from the dropzone.",
            true,
        )
        .into());
    }

    execute_file_action(
        db,
        path,
        RequestedFileAction::Rule(&rule.action),
        SourcePolicy::Dropzone,
        ActionAuditContext {
            rule_id: Some(format!("{DROPZONE_RULE_AUDIT_ID_PREFIX}{}", rule.id)),
            rule_name: Some(rule.name.clone()),
            explanation: Some(explanation),
        },
    )
}

enum RequestedFileAction<'a> {
    User(UserTriageAction),
    Rule(&'a RuleAction),
}

enum SourcePolicy {
    Watched,
    Dropzone,
}

#[derive(Default)]
struct ActionAuditContext {
    rule_id: Option<String>,
    rule_name: Option<String>,
    explanation: Option<RuleMatchExplanation>,
}

pub(crate) struct FileActionFailure {
    pub error: AppError,
    pub audit_entry: Option<Box<AuditEntry>>,
}

impl From<AppError> for FileActionFailure {
    fn from(error: AppError) -> Self {
        Self {
            error,
            audit_entry: None,
        }
    }
}

fn execute_file_action(
    db: &Database,
    path: &str,
    requested_action: RequestedFileAction<'_>,
    source_policy: SourcePolicy,
    audit: ActionAuditContext,
) -> Result<AuditEntry, FileActionFailure> {
    let config = storage::get_config(db)?;
    let source = PathBuf::from(path);
    if !source.exists() {
        return Err(AppError::path_not_found(path).into());
    }

    match source_policy {
        SourcePolicy::Watched => PathScope::new(&config).ensure_source_scope(&source)?,
        SourcePolicy::Dropzone => {
            if !source.is_file() {
                return Err(AppError::with_details(
                    "PATH_OUT_OF_SCOPE",
                    "Only files can be changed from the dropzone. No file was changed.",
                    true,
                    path,
                )
                .into());
            }
            if matches!(
                &requested_action,
                RequestedFileAction::User(UserTriageAction::Ignore)
                    | RequestedFileAction::Rule(RuleAction::Ignore)
            ) {
                PathScope::new(&config).ensure_source_scope(&source)?;
            }
        }
    }

    let mut tracked = load_or_create_tracked(db, &source, &config)?;
    let original_tracked_path = tracked.path.clone();
    let timestamp = now_seconds();
    let prepared = prepare_file_action(&source, &config, timestamp, requested_action)?;
    let (destination_path, file_name) = match &prepared {
        PreparedFileAction::Move { destination } => (
            Some(destination.to_string_lossy().to_string()),
            destination
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(&tracked.file_name)
                .to_string(),
        ),
        _ => (None, tracked.file_name.clone()),
    };
    let mut entry = AuditEntry {
        id: Uuid::new_v4().to_string(),
        sequence: storage::audit::next_audit_sequence(db)?,
        timestamp,
        action_kind: prepared.action_kind(),
        source_path: path.to_string(),
        destination_path,
        file_name,
        size_bytes: tracked.size_bytes,
        rule_id: audit.rule_id,
        rule_name: audit.rule_name,
        explanation: audit.explanation,
        undo_status: unavailable_pending_status(),
    };
    storage::audit::append_audit_entry(db, &entry)?;

    let undo_status = match apply_prepared_file_action(&source, &mut tracked, &prepared) {
        Ok(status) => status,
        Err(error) => return Err(fail_audited(db, entry, error)),
    };

    tracked.last_user_action_at = Some(timestamp);
    entry.undo_status = undo_status;
    if let Err(error) = finalize_recorded_action(db, &original_tracked_path, &tracked, &entry) {
        return Err(fail_audited(db, entry, error));
    }

    Ok(entry)
}

fn unavailable_pending_status() -> UndoStatus {
    UndoStatus::Unavailable {
        reason: String::from("Action is recorded and awaiting finalization."),
    }
}

fn fail_audited(db: &Database, mut entry: AuditEntry, error: AppError) -> FileActionFailure {
    let reason = match &error.details {
        Some(details) => format!("{} Details: {}", error.message, details),
        None => error.message.clone(),
    };
    entry.undo_status = UndoStatus::Failed { reason };
    let _ = storage::audit::update_audit_entry(db, &entry);
    FileActionFailure {
        error,
        audit_entry: Some(Box::new(entry)),
    }
}

fn finalize_recorded_action(
    db: &Database,
    original_tracked_path: &str,
    tracked: &TrackedFile,
    entry: &AuditEntry,
) -> Result<(), AppError> {
    storage::finalize_file_action(db, original_tracked_path, tracked, entry).map_err(|error| {
        let AppError {
            code,
            message,
            details,
            ..
        } = error;
        let cause = details.unwrap_or(message);
        AppError::with_details(
            "ACTION_FINALIZATION_FAILED",
            "The file action completed, but its final database state could not be saved. The audit intent remains recorded.",
            true,
            format!("{code}: {cause}"),
        )
    })
}

enum PreparedFileAction {
    Pin,
    Snooze { until: u64 },
    Ignore,
    Move { destination: PathBuf },
    Trash,
}

impl PreparedFileAction {
    fn action_kind(&self) -> AuditActionKind {
        match self {
            Self::Pin => AuditActionKind::Pin,
            Self::Snooze { .. } => AuditActionKind::Snooze,
            Self::Ignore => AuditActionKind::Ignore,
            Self::Move { .. } => AuditActionKind::Move,
            Self::Trash => AuditActionKind::Trash,
        }
    }
}

fn prepare_file_action(
    source: &Path,
    config: &AppConfig,
    timestamp: u64,
    action: RequestedFileAction<'_>,
) -> Result<PreparedFileAction, AppError> {
    match action {
        RequestedFileAction::User(UserTriageAction::Pin) => Ok(PreparedFileAction::Pin),
        RequestedFileAction::User(UserTriageAction::Snooze { seconds }) => {
            let until = timestamp.checked_add(seconds).ok_or_else(|| {
                AppError::new(
                    "ACTION_FAILED",
                    "Snooze duration is too large. No file was changed.",
                    true,
                )
            })?;
            Ok(PreparedFileAction::Snooze { until })
        }
        RequestedFileAction::User(UserTriageAction::Ignore) => Ok(PreparedFileAction::Ignore),
        RequestedFileAction::User(UserTriageAction::MoveToSafeFolder) => prepare_move_action(
            source,
            config,
            PathBuf::from(&config.safe_folder_path),
            None,
        ),
        RequestedFileAction::User(UserTriageAction::Move { destination_folder }) => {
            prepare_move_action(source, config, PathBuf::from(destination_folder), None)
        }
        RequestedFileAction::User(UserTriageAction::TrashNow) => Ok(PreparedFileAction::Trash),
        RequestedFileAction::Rule(RuleAction::Ignore) => Ok(PreparedFileAction::Ignore),
        RequestedFileAction::Rule(RuleAction::Move {
            destination_folder,
            rename_template,
        }) => prepare_move_action(
            source,
            config,
            PathBuf::from(destination_folder),
            rename_template.as_deref(),
        ),
        RequestedFileAction::Rule(RuleAction::Trash) => Ok(PreparedFileAction::Trash),
    }
}

fn prepare_move_action(
    source: &Path,
    config: &AppConfig,
    destination_folder: PathBuf,
    rename_template: Option<&str>,
) -> Result<PreparedFileAction, AppError> {
    PathScope::new(config).validate_move_destination(&destination_folder)?;
    let destination = move_destination(source, &destination_folder, rename_template)?;
    Ok(PreparedFileAction::Move { destination })
}

fn apply_prepared_file_action(
    source: &Path,
    tracked: &mut TrackedFile,
    action: &PreparedFileAction,
) -> Result<UndoStatus, AppError> {
    match action {
        PreparedFileAction::Pin => {
            tracked.state = FileDecayState::Pinned;
            tracked.expiry = Expiry::Permanent;
            Ok(UndoStatus::Available)
        }
        PreparedFileAction::Snooze { until } => {
            tracked.freshness_at = *until;
            tracked.expiry = Expiry::SnoozedUntil(*until);
            tracked.state = FileDecayState::Fresh;
            Ok(UndoStatus::Available)
        }
        PreparedFileAction::Ignore => {
            tracked.state = FileDecayState::Ignored;
            Ok(UndoStatus::Available)
        }
        PreparedFileAction::Move { destination } => {
            let destination_folder = destination.parent().ok_or_else(|| {
                AppError::new(
                    "ACTION_FAILED",
                    "Move destination has no parent folder. No file was changed.",
                    true,
                )
            })?;
            fs::create_dir_all(destination_folder)?;
            fs::rename(source, destination)?;
            apply_tracked_destination(tracked, destination);
            Ok(UndoStatus::Available)
        }
        PreparedFileAction::Trash => {
            trash::delete(source).map_err(|error| {
                AppError::with_details(
                    "ACTION_FAILED",
                    "The file could not be moved to the Recycle Bin. No raw deletion was attempted.",
                    true,
                    error.to_string(),
                )
            })?;
            tracked.state = FileDecayState::Missing;
            Ok(recycle_bin_undo_status())
        }
    }
}

fn recycle_bin_undo_status() -> UndoStatus {
    if is_trash_supported() {
        UndoStatus::Available
    } else {
        UndoStatus::Unavailable {
            reason: String::from(
                "Recycle Bin is not supported or not fully functional on this system.",
            ),
        }
    }
}

pub(crate) fn ingest_dropzone_file_audited(
    db: &Database,
    path: &str,
    watch_target_id: &str,
) -> Result<AuditEntry, FileActionFailure> {
    let config = storage::get_config(db)?;
    let target = config
        .watch_targets
        .iter()
        .find(|target| target.enabled && target.id == watch_target_id)
        .ok_or_else(|| {
            AppError::new(
                "PATH_OUT_OF_SCOPE",
                "Selected watch target is unavailable. No file was changed.",
                true,
            )
        })?;

    let source = PathBuf::from(path);
    if !source.exists() {
        return Err(AppError::path_not_found(path).into());
    }
    if !source.is_file() {
        return Err(AppError::with_details(
            "PATH_OUT_OF_SCOPE",
            "Only files can be moved into a watch target from the dropzone. No file was changed.",
            true,
            path,
        )
        .into());
    }

    let destination_folder = PathBuf::from(&target.path);
    let destination = move_destination(&source, &destination_folder, None)?;
    let original_tracked_path = source.to_string_lossy().to_string();
    let timestamp = now_seconds();
    let source_metadata = fs::metadata(&source).map_err(AppError::from)?;
    let mut entry = AuditEntry {
        id: Uuid::new_v4().to_string(),
        sequence: storage::audit::next_audit_sequence(db)?,
        timestamp,
        action_kind: AuditActionKind::Move,
        source_path: path.to_string(),
        destination_path: Some(destination.to_string_lossy().to_string()),
        file_name: destination
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(path)
            .to_string(),
        size_bytes: source_metadata.len(),
        rule_id: Some(String::from(DROPZONE_INGEST_AUDIT_ID)),
        rule_name: Some(String::from("Dropzone ingest")),
        explanation: None,
        undo_status: unavailable_pending_status(),
    };
    storage::audit::append_audit_entry(db, &entry)?;

    if let Err(error) = fs::create_dir_all(&destination_folder).map_err(AppError::from) {
        return Err(fail_audited(db, entry, error));
    }
    if let Err(error) = fs::rename(&source, &destination).map_err(AppError::from) {
        return Err(fail_audited(db, entry, error));
    }

    let metadata = match fs::metadata(&destination).map_err(AppError::from) {
        Ok(metadata) => metadata,
        Err(error) => return Err(fail_audited(db, entry, error)),
    };
    let mut tracked =
        tracked_file_from_metadata(&destination, &metadata, None, &config, &target.id);
    tracked.last_user_action_at = Some(timestamp);

    entry.undo_status = UndoStatus::Available;
    if let Err(error) = finalize_recorded_action(db, &original_tracked_path, &tracked, &entry) {
        return Err(fail_audited(db, entry, error));
    }

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
    let from_dropzone = is_dropzone_audit_entry(entry);
    let destination = entry.destination_path.as_ref().ok_or_else(|| {
        AppError::new(
            "UNDO_UNAVAILABLE",
            "The audit entry did not record a destination path.",
            true,
        )
    })?;
    let from = PathBuf::from(destination);
    let to = PathBuf::from(&entry.source_path);
    if !from_dropzone {
        validate_move_source_for_undo(&from, &config)?;
    }
    let to_parent = to.parent().ok_or_else(|| {
        AppError::new(
            "UNDO_FAILED",
            "Original path has no parent folder. No file was changed.",
            true,
        )
    })?;
    if !from_dropzone {
        PathScope::new(&config).ensure_restore_parent_scope(to_parent)?;
    }

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
        storage::tracked::replace_tracked_file(db, destination, &tracked)?;
    }
    Ok(())
}

fn is_dropzone_audit_entry(entry: &AuditEntry) -> bool {
    entry.rule_id.as_deref() == Some(DROPZONE_INGEST_AUDIT_ID)
        || entry
            .rule_id
            .as_deref()
            .is_some_and(|id| id.starts_with(DROPZONE_RULE_AUDIT_ID_PREFIX))
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
        path, &metadata, None, config, "",
    ))
}

fn validate_move_source_for_undo(path: &Path, config: &AppConfig) -> Result<(), AppError> {
    if path.exists() && PathScope::new(config).is_in_enabled_watch_target(path) {
        return Err(AppError::path_out_of_scope(path.to_string_lossy().as_ref()));
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

    let file_name = if rendered.trim().is_empty() {
        current_name
            .trim_matches(' ')
            .trim_end_matches('.')
            .to_string()
    } else {
        rendered.trim_matches(' ').trim_end_matches('.').to_string()
    };
    let file_name = if file_name.is_empty() {
        String::from("renamed-file")
    } else {
        file_name
    };
    validate_reserved_name(&file_name)?;
    Ok(file_name)
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
        validate_reserved_name(template.trim_matches(' ').trim_end_matches('.'))?;
    }

    Ok(())
}

fn validate_reserved_name(file_name: &str) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
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
    }

    let _ = file_name;
    Ok(())
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

    use crate::models::{
        AppError, AuditActionKind, AuditEntry, FileDecayState, UndoStatus, UserTriageAction,
    };
    use crate::storage;
    use crate::storage::test_util::{path_string, Fixture};
    use crate::storage::Database;

    use super::{
        execute_triage_action_audited, ingest_dropzone_file_audited, render_rename_template,
    };

    fn execute_triage_action(
        db: &Database,
        path: &str,
        action: UserTriageAction,
    ) -> Result<AuditEntry, AppError> {
        execute_triage_action_audited(db, path, action).map_err(|failure| failure.error)
    }

    fn ingest_dropzone_file(
        db: &Database,
        path: &str,
        watch_target_id: &str,
    ) -> Result<AuditEntry, AppError> {
        ingest_dropzone_file_audited(db, path, watch_target_id).map_err(|failure| failure.error)
    }

    #[test]
    fn manual_move_preserves_name_avoids_collision_and_updates_tracked_path() {
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
        .expect("move should succeed");

        let destination = entry
            .destination_path
            .expect("destination should be recorded");
        assert!(Path::new(&destination).exists());
        assert!(destination.ends_with("report-1.txt"));
        assert!(!source.exists());
        assert!(existing_destination.exists());
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
    fn failed_filesystem_action_keeps_write_ahead_audit() {
        let fixture = Fixture::new("shelflife-test");
        let source = fixture.write_watch_file("report.txt", "download");
        fixture.save_config();
        let destination_folder = fixture.outside.join("not-a-folder");
        fixture.write_file(&destination_folder, "blocking file");

        execute_triage_action(
            &fixture.db,
            &path_string(&source),
            UserTriageAction::Move {
                destination_folder: path_string(&destination_folder),
            },
        )
        .expect_err("move into a file path should fail");

        let entries =
            storage::audit::list_audit_entries(&fixture.db).expect("write-ahead audit should load");
        assert!(source.exists());
        assert_eq!(entries.len(), 1);
        assert!(matches!(entries[0].undo_status, UndoStatus::Failed { .. }));
        assert_eq!(
            entries[0].destination_path,
            Some(path_string(&destination_folder.join("report.txt")))
        );
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

    #[test]
    fn dropzone_ingest_moves_external_file_to_watch_target_and_undo_restores() {
        let fixture = Fixture::new("shelflife-dropzone-ingest");
        let source = fixture.write_outside_file("drop.txt", "external");
        fixture.save_config();

        let entry = ingest_dropzone_file(&fixture.db, &path_string(&source), "watch")
            .expect("dropzone ingest should succeed");
        let destination = entry
            .destination_path
            .as_ref()
            .expect("destination should be recorded");

        assert_eq!(entry.action_kind, AuditActionKind::Move);
        assert!(!source.exists());
        assert!(Path::new(destination).exists());
        assert!(storage::tracked::get_tracked_file(&fixture.db, destination)
            .expect("tracked lookup should work")
            .is_some());

        let undone = super::undo_audit_entry(&fixture.db, &entry.id).expect("undo should succeed");
        assert!(matches!(undone.undo_status, UndoStatus::Completed));
        assert!(source.exists());
        assert!(!Path::new(destination).exists());
    }

    #[test]
    fn dropzone_ingest_avoids_name_collision_in_watch_target() {
        let fixture = Fixture::new("shelflife-dropzone-collision");
        fixture.write_watch_file("drop.txt", "existing");
        let source = fixture.write_outside_file("drop.txt", "external");
        fixture.save_config();

        let entry = ingest_dropzone_file(&fixture.db, &path_string(&source), "watch")
            .expect("dropzone ingest should succeed");
        let destination = entry
            .destination_path
            .expect("destination should be recorded");

        assert!(destination.ends_with("drop-1.txt"));
        assert!(Path::new(&destination).exists());
    }

    #[test]
    fn dropzone_ingest_rejects_folders_before_change() {
        let fixture = Fixture::new("shelflife-dropzone-folder-ingest");
        fixture.save_config();

        let error = ingest_dropzone_file(&fixture.db, &path_string(&fixture.outside), "watch")
            .expect_err("folders should be rejected");

        assert_eq!(error.code, "PATH_OUT_OF_SCOPE");
        assert!(fixture.outside.exists());
    }
}
