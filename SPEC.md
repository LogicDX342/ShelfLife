# shelflife Product and Technical Specification

Version: 2.0
Status: v2 planned / in progress
Primary goal: Recoverable, explainable file hygiene for desktop clutter with interactive dropzone and archiving
Target platforms: Windows (macOS and Linux deferred to future phases)
Core stack: Tauri v2, Rust, Svelte 5, SQLite

---

## 0. Product Summary

shelflife is a lightweight desktop hygiene application that helps users manage temporary file clutter in folders such as Downloads and Desktop. It watches selected folders, detects files that appear stale, explains why they were flagged, and lets the user safely triage them through reversible actions.

The product is intentionally cautious. Version 1 does not behave like an aggressive cleaner. It behaves like a file triage assistant: it observes, classifies, explains, asks, acts, and logs.

### Core promise

shelflife helps users reduce clutter without losing trust in their filesystem.

### Target Features for v2

Version 2 introduces:

- **ZIP Archiving**: Non-destructive monthly compression/consolidation of stale files under a user-defined archive folder.
- **Interactive Desktop Overlay (Dropzone)**: Transparent always-on-top desktop dropzone for immediate file ingestion and triage.
- **Resource Limit Safety**: Automated CPU and battery checking to throttle background scans.

Current codebase status:

- Dropzone windowing, shake-to-dropzone monitoring, preview grouping, ingest, and rule-group execution are implemented.
- ZIP archiving is not yet represented in `RuleAction`, `AuditActionKind`, or the executor.
- Resource limit safety is not yet implemented; `sysinfo` or an equivalent resource adapter is not currently a backend dependency.

### Non-goals for v2

The second release does not include:

- OCR extraction.
- WebP conversion or other image transcoding.
- Asset metadata stripping.
- Process runtime diagnostics details in the UI.
- Multi-platform support (macOS/Linux) for advanced features.

---

## 1. Design Principles

### 1.1 Safety first

The application must never surprise the user with irreversible changes. All file-changing operations must be explicit, logged, and reversible where technically possible.

### 1.2 Explainability

Every proposed action must have a clear reason. The UI should answer:

- Why is this file shown?
- Which rule matched it?
- What action is proposed?
- What prevented an action, if anything?
- Can this be undone?

### 1.3 Low overhead

The Rust core should remain idle most of the time. File events are used as the primary trigger, with low-frequency reconciliation scans for correctness.

### 1.4 Ambient urgency

The interface should avoid countdown timers and real-time anxiety. File state is represented through calm status tiers: Fresh, Stale, and Decaying.

### 1.5 User-controlled automation

Automation is earned gradually. New rules begin in PreviewOnly mode. The user may promote a rule to AskFirst or Automatic after observing correct behavior.

---

## 2. Architecture Overview

```text
+----------------------------------------------------------+
|                    Svelte 5 Frontend                     |
|                                                          |
|  Dashboard UI                                            |
|  Ambient decay cards                                     |
|  Rule editor                                             |
|  Review queue                                            |
|  Audit and undo UI                                       |
+---------------------------+------------------------------+
                            |
                            | Tauri IPC: commands and events
                            |
+---------------------------v------------------------------+
|                     Tauri v2 Core                        |
|                                                          |
|  AppRuntime lifecycle state                              |
|  Tray menu                                               |
|  Window lifecycle                                        |
|  Notifications                                           |
|  Scoped file commands                                    |
+---------------------------+------------------------------+
                            |
                            |
+---------------------------v------------------------------+
|                 Rust File Hygiene Engine                 |
|                                                          |
|  notify watcher signal source                            |
|  debounced event queue                                   |
|  quiescence detector                                     |
|  compiled rule-set module                                |
|  safe action executor                                    |
|  audit ledger                                            |
+---------------------------+------------------------------+
                            |
                            |
+---------------------------v------------------------------+
|                   Embedded Storage                       |
|                                                          |
|  SQLite tables                                           |
|  normalized records                                      |
|  JSON config import/export                               |
+----------------------------------------------------------+
```

---

## 3. Technology Stack

### 3.1 Frontend

- Svelte 5 (utilizing `$state`, `$derived`, and `$effect` runes exclusively).
- Tailwind CSS v4 for styling.
- OS-adaptive Fluent Design guidelines (Mica material styling, custom scrollbars, toggle switches).
- Centralized reactive internationalization (i18n) translation registry (supporting English and Simplified Chinese).
- Dynamic theme switching (supporting Manual Light/Dark and System Sync settings).
- Central live snapshot adapter for Tauri IPC events, startup snapshots, focus refresh, and listener cleanup.
- Coalesced UI refresh pulses on backend events and window focus.
- No high-frequency ticking timers.

### 3.2 Backend

- Tauri v2.
- Rust.
- notify-debouncer-full for filesystem watching and event normalization; it re-exports notify.
- SQLite via Diesel for embedded storage.
- Normalized SQLite tables for internal records.
- serde for IPC model serialization and serde_json for Tauri context generation.
- trash crate or platform-specific equivalent for OS Trash/Recycle Bin behavior.
- regex and globset for pattern matching.

### 3.3 Avoided in v2

- tesseract-sys or other native OCR bindings.
- Heavy thumbnail services.
- Bundled ML models.
- Image transcoding or EXIF metadata stripping.

---

## 4. Core User Flows

### 4.1 First-run onboarding

1. User opens shelflife.
2. User selects folders to watch, such as Downloads and Desktop.
3. User chooses a default decay period.
4. App explains that new rules begin in PreviewOnly mode.
5. App performs an initial scan.
6. App shows a read-only review queue before enabling actions.

### 4.2 Daily review

1. User opens the dashboard from the tray.
2. Files appear as Fresh, Stale, or Decaying.
3. Each file card explains why it appears.
4. User chooses Pin, Snooze, Move to Safe Folder, Trash Now, Ignore, or Open in Finder/Explorer/File Manager.
5. Action is recorded in the audit ledger.
6. Undo is available where technically possible.

### 4.3 Rule preview

1. User creates or edits a rule.
2. Rule starts in PreviewOnly mode.
3. shelflife records what the rule would have done.
4. User reviews outcomes.
5. User may promote the rule to AskFirst or Automatic.

### 4.4 Manual safe cleanup

1. User selects multiple stale files.
2. User previews proposed actions.
3. App shows total file size, destination, and undo limitations.
4. User confirms.
5. App executes and logs every transaction.

---

## 5. File Monitoring Engine

### 5.1 Watch targets

Users can define watch targets. Typical defaults:

```text
~/Downloads
~/Desktop
```

Each watch target has:

- Path.
- Enabled flag.
- Default TTL.
- Rule set.
- Ignore patterns.
- Scope restrictions.

### 5.2 Event model

The watcher is a signal source, not the only source of truth.

The runtime and engine cooperate in three layers:

```text
1. Filesystem events: fast signals from notify.
2. Quiescence detection: watcher emits stable changed paths only.
3. Reconciliation scans: runtime reconciles stable paths or full watch targets.
```

The watcher must not own database access. It debounces file events, waits for path stability, and emits stable paths. `AppRuntime` owns the database handle and calls reconciliation logic with those paths.

### 5.3 Debounce and event normalization

A minimum 500 ms debounce window is required. The implementation should prefer a robust debouncer that handles:

- Rename stitching.
- Duplicate create events.
- Create/modify coalescing.
- Remove event consolidation.
- Burst downloads.
- Editor atomic-save behavior.

### 5.4 Quiescence detection

Before a file is indexed as actionable, it must be stable.

A file is considered stable when:

```text
metadata.len is unchanged across two checks
metadata.modified is unchanged across two checks
path still exists
path is not in the transient ignore list
```

Suggested checks:

```text
check 1: after debounced event
check 2: 1000 to 2000 ms later
```

### 5.5 Transient ignore list

The engine should ignore common partial or system files:

```text
*.crdownload
*.part
*.tmp
*.download
*.swp
*.lock
.DS_Store
Thumbs.db
~$*
```

### 5.6 Reconciliation scan

The reconciliation scan runs:

- On app startup.
- After watch target changes.
- After crash recovery.
- At a low-frequency interval, such as every 6 to 24 hours.

It detects files that were missed by filesystem events and removes stale tracked rows for files that no longer exist.

---

## 6. Freshness and Expiry Model

The app must not depend on a single OS timestamp. Creation date and last-accessed date may be missing or unreliable depending on platform, filesystem, or settings.

### 6.1 Stored metadata

Each tracked file stores app-level metadata:

```rust
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct TrackedFile {
    pub path: String,
    pub file_name: String,
    pub watch_target_id: String,
    pub size_bytes: u64,
    pub first_seen_at: u64,
    pub last_observed_mtime: Option<u64>,
    pub last_observed_atime: Option<u64>,
    pub last_user_action_at: Option<u64>,
    pub freshness_at: u64,
    pub expiry: Expiry,
    pub state: FileDecayState,
    pub matched_rule_ids: Vec<String>,
    pub origin: OriginEvidence,
}
```

### 6.2 Freshness formula

```text
freshness_at = max(
  first_seen_at,
  last_observed_mtime if present,
  last_observed_atime if platform marked reliable,
  last_user_action_at if present
)
```

### 6.3 Expiry formula

```text
expires_at = freshness_at + effective_ttl_seconds
```

`effective_ttl_seconds` comes from the first matching enabled rule only when that effective rule can change the file and is not `PreviewOnly` or `Ignore`. `PreviewOnly`, `Ignore`, and unmatched files use the app default TTL.

Pinned files use `Expiry::Permanent`.

When a file uses a rule-specific TTL, its stale and decaying thresholds inherit the same shape as the global timeline instead of using the global durations directly:

```text
effective_stale_threshold =
  effective_ttl_seconds * stale_threshold_seconds / default_ttl_seconds

effective_decaying_threshold =
  effective_ttl_seconds * decaying_threshold_seconds / default_ttl_seconds
```

Non-zero scaled thresholds clamp to at least 1 second.

### 6.4 Decay states

```rust
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum FileDecayState {
    Fresh,
    Stale,
    Decaying,
    Pinned,
    Ignored,
    Missing,
}
```

Default state thresholds:

```text
Fresh: first seen or active within the configured stale threshold (default 5 days)
Stale: no observed activity beyond the stale threshold (default after 5 days)
Decaying: expiry is within the configured warning window (default 24 hours)
Pinned: user has explicitly protected the file
Ignored: file is excluded by user or rule
Missing: tracked file no longer exists at path
```

Note: Fresh and Stale are continuous. A file transitions from Fresh to Stale once it exceeds the fresh threshold. There is no gap between the two states.

Thresholds are user-configurable.

---

## 7. Origin Tracking

Origin tracking is optional evidence. It is never required for safe operation.

### 7.1 Supported evidence sources (v1: Windows only)

Windows:

- Zone.Identifier alternate data stream when available.
- ZoneId and optional URL/referrer fields if present.

The following platforms are documented for future reference but are not implemented in v1:

macOS:

- Extended attributes.
- Metadata such as where-from values when available.

Linux:

- Extended attributes when available.
- No universal origin convention is assumed.

### 7.2 Origin model

```rust
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum OriginEvidence {
    MacWhereFroms { values: Vec<String> },
    WindowsZoneIdentifier {
        zone_id: Option<u32>,
        host_url: Option<String>,
        referrer_url: Option<String>,
    },
    LinuxXattr {
        key: String,
        value_utf8: Option<String>,
    },
    Unknown,
}
```

### 7.3 Rule behavior

A rule may match origin only when origin evidence exists. Unknown origin must not be treated as unsafe by default.

---

## 8. Rule Engine

### 8.1 Rule modes

```rust
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum RuleMode {
    PreviewOnly,
    AskFirst,
    Automatic,
}
```

Default mode for every new rule: `PreviewOnly`.

### 8.2 Actions

Current codebase:

```rust
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum RuleAction {
    Trash,
    Move {
        destination_folder: String,
        rename_template: Option<String>,
    },
    Ignore,
}
```

Planned v2 extension:

```rust
pub enum PlannedRuleAction {
    Archive {
        archive_root: String,
        compress_level: i32,
    },
}
```

Deferred actions for later releases:

```rust
pub enum DeferredRuleAction {
    ConvertImage { format: String },
    StripMetadata,
    ExtractText,
}
```

### 8.3 Conditions

```rust
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum SizeCondition {
    Any,
    LessThan(u64),
    GreaterThan(u64),
    Between { min: u64, max: u64 },
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct RuleConditions {
    pub extensions: Vec<String>,
    pub filename_globs: Vec<String>,
    pub filename_regexes: Vec<String>,
    /// Matches only when OriginEvidence contains a matching domain.
    /// Ignored (treated as not-matched) when origin is Unknown.
    pub source_domains: Vec<String>,
    pub size: SizeCondition,
}
```

### 8.4 Full rule model

```rust
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
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
```

### 8.5 Rule evaluation order

Rules are evaluated in this order:

```text
1. Validate path is inside an allowed watch target.
2. Apply explicit per-file pins and ignores.
3. Evaluate enabled rules by priority.
4. Produce explanation records and matched rule ids.
5. Select the first matching enabled rule as the effective verdict.
6. Queue proposed action based on the effective rule mode.
```

`matched_rule_ids` is informational and preserves all matching rule ids in priority order. Effective behavior comes from the rule verdict, not from reinterpreting the first stored id. A higher-priority `PreviewOnly` rule blocks lower-priority executable rules. `Ignore` takes effect immediately as `FileDecayState::Ignored` and does not apply a rule TTL.

The raw `AutomationRule` records from storage or IPC are compiled into one `CompiledRuleSet` before evaluation. Compilation is the shared rule-engine seam and must:

- Validate filename regexes and globs, source-domain patterns, size ranges, watch-path scope, move destinations, and rename templates.
- Sort rules by descending priority while preserving the stored order for equal priorities.
- Compile filename glob sets and regular expressions once for the operation snapshot.
- Keep condition matching, rule ordering, and effective-verdict selection in one implementation.

Reconciliation, incremental watcher processing, rule refresh, dropzone preview and execution, file explanations, and automatic rule scheduling all consume `CompiledRuleSet`. These workflows must not independently sort raw rules, validate rule syntax, or rebuild filename matchers. The compiled set exposes the per-file evaluation interface:

```rust
pub struct CompiledRuleSet { /* compiled rules and matchers */ }

impl CompiledRuleSet {
    pub fn compile(
        rules: impl IntoIterator<Item = AutomationRule>,
        config: &AppConfig,
    ) -> Result<Self, AppError>;

    pub fn decide_file(
        &self,
        file: &TrackedFile,
        scope: RuleDecisionScope,
    ) -> RuleDecision;

    pub fn explain_file(&self, file: &TrackedFile) -> Vec<RuleMatchExplanation>;
}
```

`RuleDecisionScope::WatchedFile` applies the rule's `watch_path`; `RuleDecisionScope::Dropzone` evaluates the compiled rule set without watched-file path filtering. Invalid rule data fails at compilation rather than during a per-file match.

Internal rule evaluation returns a decision:

```rust
pub enum RuleVerdict {
    Matched {
        effective_rule: AutomationRule,
        effective_explanation: RuleMatchExplanation,
        rule_ttl_seconds: Option<u64>,
    },
    Unmatched,
}

pub struct RuleDecision {
    pub verdict: RuleVerdict,
    pub explanations: Vec<RuleMatchExplanation>,
    pub matched_rule_ids: Vec<String>,
}
```

Automatic rule execution records failed attempts as audit entries with `UndoStatus::Failed`. A stored failed attempt for the same path and rule prevents future automatic scheduling for that pair. There is no in-memory retry/backoff state.

### 8.6 Rule explanations

Each rule evaluation emits an explanation object.

```rust
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
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
```

The UI must show this explanation before user-confirmed actions.

---

## 9. Action Execution

### 9.1 Allowed actions in v1

- Pin.
- Snooze (configurable duration: default 7 days, options include 1 day, 3 days, 7 days, 14 days, 30 days, or custom).
- Ignore.
- Move to Safe Folder.
- Trash Now.
- Open file location.
- Undo recent action where possible.

Snooze resets the file's `freshness_at` to the current time plus the chosen snooze duration, temporarily suppressing decay progression. When the snooze period expires, the file resumes normal decay evaluation.

Archive is planned for v2 and is not implemented in the current Rust action model.

### 9.2 Removed terminology

The product must not use the word "Nuke" in user-facing UI. Use "Trash Now" for OS trash behavior.

### 9.3 Trash behavior

Trash actions must move files to the operating system Trash or Recycle Bin. Raw deletion is not available in v1.

### 9.4 Safe folder behavior

The safe folder is a user-visible folder, not a hidden application cache.

Default suggestion:

```text
~/shelflife-safe
```

The app may also support platform-specific defaults selected during onboarding.

### 9.5 Move with rename behavior

Move actions can optionally rename the file using a rename template.

Default cleanup transformations when a template is used:

```text
remove duplicate suffixes: " (1)", "_copy", " copy"
normalize whitespace
optionally prepend ISO date: YYYY-MM-DD_filename.ext
```

The execution must detect name collisions in the destination folder and produce a safe alternative instead of overwriting.

### 9.6 Dry run versus staging

Dry run means no filesystem changes.

Staging means files are copied or moved only after explicit approval.

### 9.7 Archive behavior

Archive is planned v2 behavior. It will process files by packaging them into a monthly ZIP file under the user-defined archive folder (e.g., `shelflife_archive_2026-06.zip`). The original file is then safely removed, and an audit ledger entry is created allowing undo (restoring the file from the ZIP archive).

Automatic movement to an internal staging directory is not considered dry run and is not enabled by default.

---

## 10. Audit Ledger and Undo

### 10.1 Audit goals

The audit ledger records all file-changing actions for at least 30 days.

It supports:

- User trust.
- Undo.
- Crash recovery.
- Analytics.
- Debugging.

### 10.2 Audit model

```rust
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum AuditActionKind {
    Trash,
    Move,
    Pin,
    Snooze,
    Ignore,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct AuditEntry {
    pub id: String,
    pub sequence: u64,
    pub timestamp: u64,
    pub action_kind: AuditActionKind,
    pub source_path: String,
    pub destination_path: Option<String>,
    pub file_name: String,
    pub size_bytes: u64,
    pub rule_id: Option<String>,
    pub rule_name: Option<String>,
    pub explanation: Option<RuleMatchExplanation>,
    pub undo_status: UndoStatus,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum UndoStatus {
    Available,
    Unavailable { reason: String },
    Completed,
    Failed { reason: String },
}
```

### 10.3 Audit keying

Do not key audit rows only by timestamp. Multiple actions can happen in the same millisecond.

Use:

```text
monotonic sequence
or ULID
or UUID plus timestamp index
```

### 10.4 Undo limitations

Undo is best-effort. The app must be honest when undo is unavailable.

Examples:

```text
File was moved again outside shelflife.
File no longer exists in destination.
Trash location cannot be resolved.
Name collision prevents restoring original path.
Permission denied.
```

### 10.5 Write-ahead action auditing

Every file-changing action writes an audit intent before touching the file. The intent records the
source, planned destination when applicable, action kind, and rule explanation. After the
filesystem step, the tracked-file update and final audit status are committed in one database
transaction. A failed filesystem step marks the intent as failed; interruption or persistence
failure therefore leaves a durable audit record instead of an unlogged file change.

---

## 11. Storage Layer

### 11.1 Database

The app uses SQLite as an embedded relational store. Storage owns database opening, schema migrations, table definitions, and CRUD only. Runtime lifecycle state does not live in storage.

The runtime database file is:

```text
shelflife.sqlite
```

This is a breaking storage change from the earlier redb prototype. Existing `.redb` files are not imported or deleted.

Runtime state:

```rust
use crate::storage::Database;

pub struct AppRuntime {
    db: Database,
    // watcher handle, pause flag, reconciliation flag,
    // rule execution flag, exclusive engine-operation gate,
    // scheduler wake condition
}
```

`AppRuntime` is managed by Tauri and owns lifecycle orchestration:

```text
startup setup
watcher restart / pause / resume
dropzone monitor sync
startup, manual, and periodic reconciliation
automatic rule execution scheduling
runtime event emission
```

Full reconciliation, watcher-driven incremental reconciliation, and automatic rule execution
share one exclusive runtime gate. Their snapshot-and-write phases must not overlap, so a rule
action cannot move a file while reconciliation is preparing to persist stale tracked-file state.

`lib.rs` only declares modules and builds the Tauri app. Command modules validate input, persist requested config/model changes, and delegate lifecycle effects to `AppRuntime`.

### 11.2 Serialization

Internal records are fully normalized into SQLite columns and child tables. SQLite columns are the source of truth; internal storage must not duplicate full model payloads as JSON or binary blobs.

Schema versioning:

```text
PRAGMA user_version
```

Config export/import is not implemented.

IPC payloads:

```text
serde JSON-compatible structs
```

### 11.3 Tables

```text
app_config
watch_targets
watch_target_ignore_patterns
watch_target_include_hidden_patterns

automation_rules
rule_extensions
rule_filename_globs
rule_filename_regexes
rule_source_domains

tracked_files
tracked_file_rules
origin_values

audit_sequence_state
audit_entries
```

`audit_entries` stores optional `RuleMatchExplanation` fields as nullable `explanation_*` columns. There is no separate one-to-one explanation table.

### 11.4 Index strategy

Primary lookups:

```text
config singleton -> app config
file path -> tracked file
rule id -> rule
audit id -> audit entry
audit sequence -> audit entry
```

Secondary lookups:

```text
rule priority/name -> rule ordering
tracked state -> dashboard filtering
tracked expiry -> automatic scheduling
matched rule id -> tracked paths
audit sequence -> audit ordering
failed status + source path + rule id -> failed automatic attempts
```

Indexes must support common dashboard, scheduler, and audit queries without duplicating authoritative model payloads.

---

## 12. IPC Interface

The frontend does not receive broad filesystem permissions. Rust owns all file operations and exposes a narrow command surface.

### 12.1 Commands

```rust
#[tauri::command]
async fn get_active_files() -> Result<Vec<TrackedFile>, AppError>;

#[tauri::command]
async fn explain_file(path: String) -> Result<Vec<RuleMatchExplanation>, AppError>;

#[tauri::command]
async fn preview_file(path: String) -> Result<FilePreview, AppError>;

#[tauri::command]
async fn open_file_location(path: String) -> Result<(), AppError>;

#[tauri::command]
async fn execute_triage_action(
    path: String,
    action: UserTriageAction,
) -> Result<AuditEntry, AppError>;

#[tauri::command]
async fn execute_bulk_triage_action(
    paths: Vec<String>,
    action: UserTriageAction,
) -> Result<BulkTriageResult, AppError>;

#[tauri::command]
async fn undo_audit_entry(audit_id: String) -> Result<AuditEntry, AppError>;

#[tauri::command]
async fn list_audit_entries() -> Result<Vec<AuditEntry>, AppError>;

#[tauri::command]
async fn preview_dropzone_files(paths: Vec<String>) -> Result<DropzonePreview, AppError>;

#[tauri::command]
async fn execute_dropzone_ingest(
    paths: Vec<String>,
    watch_target_id: String,
) -> Result<DropzoneActionResult, AppError>;

#[tauri::command]
async fn execute_dropzone_rule_group(
    rule_id: String,
    paths: Vec<String>,
) -> Result<DropzoneActionResult, AppError>;

#[tauri::command]
async fn hide_dropzone() -> Result<(), AppError>;

#[tauri::command]
async fn list_rules() -> Result<Vec<AutomationRule>, AppError>;

#[tauri::command]
async fn save_rule(rule: AutomationRule) -> Result<AutomationRule, AppError>;

#[tauri::command]
async fn test_rule(rule: AutomationRule) -> Result<Vec<RuleMatchExplanation>, AppError>;

#[tauri::command]
async fn delete_rule(id: String) -> Result<(), AppError>;

#[tauri::command]
async fn get_config() -> Result<AppConfig, AppError>;

#[tauri::command]
async fn save_config(config: AppConfig) -> Result<AppConfig, AppError>;

#[tauri::command]
async fn resolve_close_request(behavior: CloseBehavior, remember: bool) -> Result<(), AppError>;

#[tauri::command]
async fn update_watch_targets(targets: Vec<WatchTarget>) -> Result<(), AppError>;

#[tauri::command]
async fn run_reconciliation_scan() -> Result<(), AppError>;

#[tauri::command]
async fn is_reconciliation_active() -> Result<bool, AppError>;

#[tauri::command]
async fn pause_watching() -> Result<(), AppError>;

#[tauri::command]
async fn resume_watching() -> Result<(), AppError>;

#[tauri::command]
async fn select_directory(
    title: Option<String>,
    default_path: Option<String>,
) -> Result<Option<String>, AppError>;
```

### 12.2 Events

```text
file_indexed
file_updated
file_removed
action_completed
action_failed
audit_updated
reconciliation_started
reconciliation_progress
reconciliation_completed
close_behavior_requested
```

Events are for background updates. Commands are for user-initiated requests and operations that need responses.

### 12.3 Path validation

Every command that receives a path must validate:

- Path exists, unless the action is explicitly about a missing file.
- Canonical path is inside a configured watch target or safe folder.
- Path is not a system directory.
- Path is not outside the user's approved scope.
- Symlinks do not escape allowed roots unless explicitly permitted.

---

## 13. Frontend Specification

### 13.1 Main dashboard

The dashboard shows:

- Watch target summary.
- Active files grouped by decay state.
- Total reviewed files.
- Total recoverable clutter size.
- Rule preview queue.
- Recent audit entries.

### 13.2 File card states

```text
Fresh Card:
  solid border
  calm green/blue accent
  added or active recently

Stale Card:
  amber accent
  neutral text
  no observed activity for configured stale threshold

Decaying Card:
  charcoal/low-contrast card
  soft warning bar
  expiry within configured warning window

Pinned Card:
  pinned indicator
  no automatic action

Ignored Card:
  subdued indicator
  hidden by default unless filters show ignored items
```

### 13.3 Svelte 5 state model

Frontend state should be derived from backend snapshots and events through a central live snapshot adapter.

Guidelines:

```text
Use $state for local UI state.
Use $derived for visual decay labels.
Use command invocations for authoritative data.
Use the live snapshot adapter as the only owner of backend file/audit event names.
Use events to trigger coalesced full snapshot refreshes.
Avoid interval timers faster than 15 minutes for background refresh.
Refresh on window focus through the live snapshot adapter.
Preview panel triggers use on-demand command calls, not timers.
```

Views must not register live backend listeners directly for file, audit, or reconciliation freshness. The adapter owns:

```text
initial file and audit snapshot loading
reconciliation progress state
action_completed / audit_updated / file path event handling
window focus refresh fallback
coalescing so only one refresh per snapshot kind runs at a time
listener cleanup on layout teardown
```

### 13.4 Preview panel

Preview is lazy.

Trigger:

```text
user hovers, focuses, or expands a file card
```

v1 preview types:

```text
text: first safe UTF-8 snippet
markdown: first safe UTF-8 snippet
image: metadata and optional lightweight thumbnail path
pdf: metadata only
unknown: icon and metadata only
```

Avoid sending large base64 payloads through IPC. Prefer cached preview artifacts and scoped asset loading where possible.

### 13.5 Rule explanation UI

Each proposed action must show:

```text
Rule name
Matched conditions
Proposed action
Mode: PreviewOnly, AskFirst, or Automatic
Undo availability
```

---

## 14. Tray and Window Behavior

### 14.1 Tray menu

Tray menu options:

```text
Open shelflife
Review decaying files
Pause watching
Resume watching
Run reconciliation scan
Preferences
Quit
```

### 14.2 Dashboard window

The dashboard window is the primary UI. It may be hidden instead of destroyed, but memory behavior must be measured on each platform.

### 14.3 Dropzone behavior

The dropzone is an optional, transparent desktop overlay:

- Rendered in its own transparent always-on-top Tauri window at `/dropzone`.
- When enabled, dragging files and shaking the cursor shows the dropzone near the cursor.
- Dropped files are previewed, grouped by effective rule, and can be moved into a watch target or processed through executable rule groups.
- PreviewOnly rule groups are shown for explanation but cannot change files.
- Technical implementation requires:
  ```text
  transparent: true
  decorations: false
  alwaysOnTop: true
  skipTaskbar: true
  ```

---

## 15. Notifications

Notifications should be sparse and useful.

Allowed v1 notifications:

```text
Files are ready for review.
A rule preview found files that would be affected.
An action completed.
An action failed.
Undo failed or needs user attention.
```

Avoid notifications for every file event.

Notification text example:

```text
5 files are ready for review in Downloads. No files were moved automatically.
```

---

## 16. Security and Permissions

### 16.1 Principle

The frontend receives minimal permissions. Rust validates all filesystem operations.

### 16.2 Tauri capabilities

Create separate capabilities for:

```text
main dashboard window
dropzone window, future
preferences window, optional
```

The dropzone capability must have minimal command access.

### 16.3 Command scopes

Custom commands must enforce scopes internally. The app must not rely only on frontend behavior.

### 16.4 Sensitive paths

The app should block or warn before acting on:

```text
home directory root
system directories
application directories
cloud sync roots unless approved
source code repositories unless approved
external drives unless approved
hidden files unless approved
```

### 16.5 Symlink and canonicalization rules

Before action execution:

```text
canonicalize source path
canonicalize destination parent
verify both are inside approved roots
reject symlink escapes by default
record original and canonical paths in audit metadata
```

---

## 17. Error Handling

### 17.1 Error model

```rust
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub recoverable: bool,
    pub details: Option<String>,
}
```

### 17.2 Common error codes

```text
PATH_OUT_OF_SCOPE
PATH_NOT_FOUND
PERMISSION_DENIED
FILE_NOT_STABLE
RULE_INVALID_REGEX
RULE_INVALID_DESTINATION
ACTION_FAILED
UNDO_UNAVAILABLE
UNDO_FAILED
DATABASE_ERROR
WATCHER_ERROR
```

### 17.3 User-facing error style

Errors must state:

```text
what failed
whether the file was changed
how to recover
whether undo is available
```

---

## 18. Analytics and Statistics

v1 statistics are derived from audit and tracked-file tables.

### 18.1 Included metrics

```text
Total size moved to Trash
Total size moved to Safe Folder
Total files pinned
Total files ignored
Current stale size by watch target
Current decaying size by watch target
Workspace composition by extension
Rule preview accuracy, manual review based
```

### 18.2 Excluded from v2

```text
OCR text extraction metrics
Image conversion savings
Metadata stripping savings
Process runtime diagnostics detail
```

### 18.3 Computation strategy

Use indexed table reads where possible. Avoid expensive dashboard-triggered full scans.

Background aggregation can update a future normalized `stats` table.

---

## 19. Configuration

### 19.1 Config model

```rust
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum CloseBehavior {
    Ask,
    HideToTray,
    Quit,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub watch_targets: Vec<WatchTarget>,
    pub default_ttl_seconds: u64,
    pub stale_threshold_seconds: u64,
    pub decaying_threshold_seconds: u64,
    pub safe_folder_path: String,
    pub notifications_enabled: bool,
    pub start_at_login: bool,
    pub close_behavior: CloseBehavior,
    pub dropzone_enabled: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WatchTarget {
    pub id: String,
    pub path: String,
    pub enabled: bool,
    pub recursive: bool,
    pub ignore_patterns: Vec<String>,
    pub include_hidden_patterns: Vec<String>,
}
```

### 19.2 Defaults

```text
default_ttl_seconds: 30 days
stale_threshold_seconds: 5 days
decaying_threshold_seconds: 24 hours
notifications_enabled: true
start_at_login: false
close_behavior: Ask
dropzone_enabled: false
recursive: false for Desktop, true or false user-selected for Downloads
```

---

## 20. Testing Strategy

### 20.1 Rust unit tests

Test:

```text
rule matching
compiled rule-set validation, ordering, and matcher reuse
freshness calculation
expiry calculation
path scope validation
symlink rejection
move destination collision handling
audit row creation
undo state transitions
storage schema migrations and model round-trips
```

### 20.2 Integration tests

Test with temporary directories:

```text
create file
modify file
rename file
delete file
partial download simulation
rapid event bursts
watcher stable-path emission without database access
move to safe folder
trash action mock or platform-gated test
undo move (including with rename)
missing file reconciliation
PreviewOnly blocks lower-priority Automatic rules
Ignore rules apply immediately without rule TTL
effective rule TTL versus app default TTL
```

### 20.3 Platform tests

macOS:

```text
FSEvents behavior
Trash behavior
xattr origin evidence
notarized build behavior
```

Windows:

```text
ReadDirectoryChangesW behavior
Recycle Bin behavior
Zone.Identifier parsing
long path behavior
SmartScreen expectations
```

Linux:

```text
inotify behavior
Trash specification behavior
xattr availability
network filesystem fallback behavior
```

### 20.4 UI tests

Test:

```text
dashboard grouping
rule explanation rendering
confirmation dialogs
audit list
undo status display
empty states
error states
reduced-motion mode
```

---

## 21. Distribution and Updates

### 21.1 Signing

The product requires a signing plan before public distribution.

Required planning:

```text
macOS code signing
macOS notarization
Windows code signing
Linux package metadata
update signing keys
key storage and rotation policy
```

### 21.2 Updates

App updates must be signed. The app uses Tauri v2's Rust updater API with signed updater
artifacts published from GitHub Releases. The frontend talks to ShelfLife-owned IPC commands
only; updater authority and signature verification stay in the Rust layer.

Updater behavior is conservative:

- ShelfLife checks once per main-window session and allows manual checks from About.
- Users must explicitly choose to install an available update.
- On Windows, the UI warns that ShelfLife exits while the installer applies the update.
- The updater private key must be stored only in release secrets; only the public key is
  checked into `tauri.conf.json`.

### 21.3 Privacy posture

shelflife operates locally by default.

v1 must not upload:

```text
file names
file paths
file contents
origin URLs
usage analytics
```

Any future telemetry must be explicit opt-in.

---

## 22. MVP Roadmap

### Phase 0: Rust-only safety prototype

Goal: prove the core file model without UI.

Deliverables:

```text
SQLite initialization
watch target config
notify watcher
500 ms debounced event queue
quiescence detection
initial reconciliation scan
tracked file records
console rule explanations
no file-changing actions
```

Exit criteria:

```text
Can watch one folder.
Can detect stable files.
Can classify Fresh, Stale, Decaying.
Can persist and reload tracked file records.
Can recover after restart.
```

### Phase 1: Tray and read-only dashboard

Goal: make the app visible and understandable.

Deliverables:

```text
Tauri tray
Svelte dashboard
active file list
ambient decay cards
watch target preferences
rule explanation display
read-only audit preview
```

Exit criteria:

```text
User can see what shelflife would manage.
No automatic file changes occur.
Dashboard does not require high-frequency timers.
```

### Phase 2: Manual triage and audit ledger

Goal: enable safe user-confirmed actions.

Deliverables:

```text
Pin
Snooze
Ignore
Move to Safe Folder
Trash Now
AuditEntry records
Undo for move (including with rename)
Best-effort undo for trash
```

Exit criteria:

```text
Every file-changing action creates an audit row.
Undo status is visible.
Errors state whether a file was changed.
```

### Phase 3: Rule preview mode

Goal: let users test automation without risk.

Deliverables:

```text
rule editor
condition matching
PreviewOnly mode
rule test command
preview queue
notification for reviewable previews
```

Exit criteria:

```text
Rules can explain what they would do.
No preview rule changes files.
User can promote a rule manually.
```

### Phase 4: Limited automation

Goal: allow trusted rules to act.

Deliverables:

```text
AskFirst mode
Automatic mode
per-rule enable switch
per-rule action limits
automation pause control
bulk undo visibility
```

Exit criteria:

```text
Automatic rules are opt-in.
User can pause automation immediately.
All automated actions are auditable.
```

### Phase 5: v2 Features (Dropzone, ZIP Archiving, and Resource Limits)

Goal: Implement desktop dropzone, ZIP archiving, and automated resource limit safety.

Deliverables:

- Transparent dropzone window with drag-and-drop file ingestion.
- ZIP archiving action within rules and triage flow.
- CPU usage checking using `sysinfo` to throttle scans. Battery-specific throttling remains planned.

Exit criteria:

- Dropzone window compiles, can be toggled, and detects dragged files.
- Stale files can be automatically or manually ZIP-archived.
- Background scans pause when CPU usage exceeds 70%.

---

## 23. Acceptance Criteria for v1

v1 is implemented when:

```text
App can watch configured folders.
App can index files without polling aggressively.
Watcher emits stable file paths without opening storage directly.
App can recover missed events through reconciliation.
Runtime owns reconciliation and automatic rule scheduling lifecycle.
Frontend centralizes live file and audit snapshots behind one adapter.
App can classify files into ambient decay states.
App can explain every proposed action.
App can perform user-confirmed move (with optional rename), pin, ignore, snooze, and trash actions.
App logs every file-changing action.
App exposes undo status for every logged action.
App does not perform automatic destructive actions by default.
App keeps frontend filesystem permissions narrow.
App validates all paths in Rust.
```

### 23.1 v1 implementation status

Status: implemented.

## 23.2 Acceptance Criteria for v2

v2 is implemented when:

```text
Transparent dropzone window displays and correctly ingests dragged files.
ZIP archiving compression packages files correctly.
Background scans and file indexing respect resource limit controls (spikes above 70% CPU throttle the engine).
Code builds and tests successfully on Windows 11.
```

### 23.3 v2 implementation status

Status: in progress.

Current codebase:

```text
Implemented: dropzone window, shake monitor, preview grouping, watch-target ingest, rule-group execution.
Not implemented: ZIP archive RuleAction/AuditActionKind/executor behavior.
Implemented: CPU-aware periodic reconciliation defers scans above 70% system usage and retries after five minutes.
Not implemented: battery resource throttling.
```

Validation commands:

```text
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
pnpm lint
pnpm check
pnpm tauri build --no-bundle
```

Run the no-bundle Windows build from the repository root:

```powershell
.\src-tauri\target\release\shelflife.exe
```

For development with the Svelte/Vite frontend and Tauri backend:

```powershell
pnpm tauri dev
```

To rebuild the release executable without producing an installer bundle:

```powershell
pnpm tauri build --no-bundle
```

---

## 24. Open Engineering Questions

These should be resolved during Phase 0 and Phase 1:

```text
Which debounce implementation produces the fewest false positives per platform?
How reliable is trash undo on each supported OS?
Should cloud-synced subfolders within Downloads be auto-detected and excluded?
How should cloud-synced folders be detected and warned about?
Which preview formats are safe and useful without heavy dependencies?
What is the minimum notification frequency users tolerate?
```

---

## 25. Naming and Copy Guidelines

Preferred language:

```text
Review
Trash Now
Move to Safe Folder
Pin
Snooze
Ignore
PreviewOnly
AskFirst
Automatic
Undo available
Undo unavailable
```

Avoid:

```text
Nuke
Destroy
Purge
Erase forever
Clean automatically
Delete silently
```

The product should sound calm, precise, and reversible.

---

## 26. Summary

shelflife is a local-first desktop file hygiene assistant. The v2 specification prioritizes user trust, clear explanations, recoverable actions, and conservative automation. The revised product targets drag-and-drop workspace interaction via a desktop dropzone overlay, safe compression optimizations via monthly ZIP archiving, and strict background resource throttling to ensure minimal host system footprint.
