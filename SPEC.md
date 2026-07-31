# shelflife Product and Technical Specification

Version: 2.1
Status: v2 in progress
Primary goal: Recoverable, explainable file hygiene for desktop clutter with an interactive dropzone and resource-aware automation
Target platform: Windows (macOS and Linux deferred)
Core stack: Tauri v2, Rust, Svelte 5, SQLite (Diesel)

---

## 0. Product Summary

shelflife is a lightweight desktop hygiene application that helps users manage temporary file clutter in folders such as Downloads and Desktop. It watches selected folders, detects files that appear stale, explains why they were flagged, and lets the user safely triage them through reversible actions.

The product is intentionally cautious. It behaves like a file triage assistant: it observes, classifies, explains, asks, acts, and logs. shelflife helps users reduce clutter without losing trust in their filesystem.

### v2 additions

- **Interactive Desktop Overlay (Dropzone)**: Transparent always-on-top overlay for immediate file ingestion and triage via drag-and-drop with shake-to-reveal.

---

## 1. Design Principles

### 1.1 Safety first

The application must never surprise the user with irreversible changes. All file-changing operations must be explicit, logged, and reversible where technically possible.

### 1.2 Explainability

Every proposed action must have a clear reason. The UI must answer: Why is this file shown? Which rule matched? What action is proposed? What prevented action? Can this be undone?

### 1.3 Low overhead

The Rust core should remain idle most of the time. File events are the primary trigger, with low-frequency reconciliation scans for correctness.

### 1.4 Ambient urgency

The interface avoids countdown timers and real-time anxiety. File state is represented through calm status tiers: Fresh, Stale, and Decaying.

### 1.5 User-controlled automation

Automation is earned gradually. Manually created rules begin in PreviewOnly and may be promoted after review. Starter templates use predefined AskFirst or Automatic modes and timing.

---

## 2. Core User Flows

### First-run onboarding

- User selects folders to watch (e.g. Downloads, Desktop).
- User chooses a default decay period.
- App explains that manually created rules begin in PreviewOnly mode.
- App performs an initial scan and shows a read-only review queue.

### Daily review

- User opens the dashboard from the tray.
- Files appear as Fresh, Stale, or Decaying with explanations.
- User chooses Pin, Snooze, Move, Trash Now, Ignore, or Open in Explorer.
- Action is recorded in the audit ledger; undo is available where possible.

### Rule preview

- User creates or edits a manual rule (starts in PreviewOnly mode).
- shelflife records what the rule would have done.
- User reviews outcomes and may promote to AskFirst or Automatic.

### Manual safe cleanup

- User selects multiple stale files.
- App shows total file size, destination, and undo limitations.
- User confirms; app executes and logs every transaction.

---

## 3. File Monitoring Engine

### Watch targets

Each watch target has: path, enabled flag, default TTL, rule set, ignore patterns, and scope restrictions.

### Event model

The watcher is a signal source, not the only source of truth. Three layers cooperate:

1. **Filesystem events**: fast signals from notify.
2. **Quiescence detection**: watcher emits stable changed paths only.
3. **Reconciliation scans**: runtime reconciles stable paths or full watch targets.

The watcher must not own database access. It debounces file events, waits for path stability, and emits stable paths. `AppRuntime` owns the database handle and calls reconciliation logic with those paths.

### Debounce

Minimum 500 ms debounce window. Must handle: rename stitching, duplicate create events, create/modify coalescing, remove consolidation, burst downloads, and editor atomic-save behavior.

### Quiescence detection

A file is considered stable when its size and mtime are unchanged across two checks (second check 1–2 seconds after debounced event), the path still exists, and the path is not in the transient ignore list.

### Transient ignore list

Common partial or system files to ignore: `*.crdownload`, `*.part`, `*.tmp`, `*.download`, `*.swp`, `*.lock`, `.DS_Store`, `Thumbs.db`, `~$*`.

### Reconciliation scan

Runs on: app startup, watch target changes, crash recovery, and at a low-frequency interval (6–24 hours). Detects missed files and removes stale tracked rows for files that no longer exist.

`tracked_files` contains only files currently inside enabled watch targets. Files are removed from tracking when they disappear, leave a watch target, are moved, or are trashed. Audit entries retain completed-action history. Undo recreates tracking only when the restored path is inside an enabled watch target.

---

## 4. Freshness and Expiry Model

The app must not depend on a single OS timestamp because filesystem timestamps may be missing or unreliable.

### Freshness formula

On first discovery, `freshness_at` is the maximum of the current time and the file's modification time when present. Later reconciliation keeps freshness monotonic by taking the maximum of the stored `freshness_at` and the latest observed modification time. Explicit user actions update `freshness_at` directly; snoozing sets it to the snooze deadline.

### Expiry formula

```
expires_at = freshness_at + effective_ttl_seconds
```

`effective_ttl_seconds` comes from the first matching enabled `AfterSeconds` rule only when that rule can change the file and is not PreviewOnly or Ignore. `OnArrival`, PreviewOnly, Ignore, and unmatched files use the app default TTL. Pinned files use `Expiry::Permanent`.

### Threshold scaling

When a file uses a rule-specific TTL, its stale and decaying thresholds scale proportionally from the global timeline:

```
effective_stale_threshold = effective_ttl * stale_threshold / default_ttl
effective_decaying_threshold = effective_ttl * decaying_threshold / default_ttl
```

Non-zero scaled thresholds clamp to at least 1 second.

### Decay states

- **Fresh**: active within the configured stale threshold (default 5 days).
- **Stale**: no activity beyond the stale threshold.
- **Decaying**: expiry within the configured warning window (default 24 hours).
- **Pinned**: user-protected, no automatic action.
- **Ignored**: excluded by user or rule, hidden by default.

Fresh and Stale are continuous with no gap between them. Thresholds are user-configurable.

---

## 5. Origin Tracking

Origin tracking is optional evidence. It is never required for safe operation.

### Windows implementation

Read the Zone.Identifier alternate data stream when available. Use the first valid HTTP(S) URL from `HostUrl`, falling back to `ReferrerUrl`. ZoneId is not stored.

### Storage

All platform metadata reduces to `origin_url: Option<String>`. URLs are parsed as HTTP(S), reduced to `scheme://host[:non-default-port]/`, and stored without credentials, path, query, or fragment. Invalid or unavailable evidence becomes `None`.

### Rule behavior

A rule may match origin only when `origin_url` contains a matching domain. Absent origin must not be treated as unsafe by default.

---

## 6. Rule Engine

### Rule modes

- **PreviewOnly**: default for manually created rules. Records what would happen, changes nothing.
- **AskFirst**: proposes actions that require user confirmation.
- **Automatic**: acts without user confirmation when the rule's timing becomes eligible.

### Rule timing

- **OnArrival**: for Automatic Move rules only. After an incremental watcher event indexes a new stable file, Automatic rules move it immediately. Full startup/manual reconciliation scans and existing tracked files do not trigger arrival actions, and the rule does not replace their decay TTL.
- **AfterSeconds**: applies the configured rule TTL and becomes eligible when the file expires. This is the default for existing and manually created rules.

### Actions

- **Trash**: move to OS Recycle Bin.
- **Move**: move to a user-selected destination, with optional rename template.
- **Ignore**: exclude from decay tracking.

### Evaluation order

1. Validate path is inside an allowed watch target.
2. Apply explicit per-file pins and ignores.
3. Evaluate enabled rules by priority (descending, stable order for ties).
4. Produce explanation records and matched rule ids.
5. Select the first matching enabled rule as the effective verdict.
6. Queue proposed action based on the effective rule mode and timing.

`matched_rule_ids` is informational and preserves all matching rule ids in priority order. Effective behavior comes from the rule verdict. A higher-priority PreviewOnly rule blocks lower-priority executable rules. Ignore takes effect immediately as `FileDecayState::RuleIgnored` and does not apply a rule TTL. A direct user ignore uses `FileDecayState::ManuallyIgnored` and persists independently of rule projection.

### Compiled rule-set seam

Raw `AutomationRule` records are compiled into one `CompiledRuleSet` before evaluation. Compilation validates regexes, globs, source-domain patterns, size ranges, watch-path scope, move destinations, and rename templates. It sorts rules by descending priority and compiles glob sets and regex patterns once.

All workflows — reconciliation, incremental watcher processing, arrival execution, rule refresh, dropzone preview/execution, file explanations, and expiry scheduling — consume `CompiledRuleSet`. They must not independently sort raw rules, validate syntax, or rebuild matchers. Invalid rule data fails at compilation rather than during per-file match.

`RuleDecisionScope::WatchedFile` applies the rule's `watch_path`; `RuleDecisionScope::Dropzone` evaluates without watched-file path filtering.

### Automatic execution failures

Failed automatic attempts are stored as audit entries with `UndoStatus::Failed`. A stored failed attempt for the same path and rule prevents future automatic scheduling for that pair. There is no in-memory retry/backoff state.

---

## 7. Action Execution

### Allowed actions

Pin, Snooze (configurable: 1d, 3d, 7d, 14d, 30d, custom), Ignore, Move to a user-selected destination, Trash Now, Open file location, Undo where possible.

### Snooze behavior

Snooze resets `freshness_at` to the current time plus the chosen duration, temporarily suppressing decay. When the snooze expires, normal decay evaluation resumes.

### Trash behavior

Trash actions must move files to the OS Recycle Bin. Raw deletion is not available.

### Move behavior

- Users choose a destination for every manual move. An optional default and frontend-only recent destinations make common folders quick to reuse.
- Moving a file outside its watch target completes its tracked lifecycle. The destination is user-visible and undoable through audit, but the file is not retained in `tracked_files`.
- Move actions can optionally rename using a template (duplicate suffix removal, whitespace normalization, optional ISO date prefix).
- Name collisions in the destination folder must produce a safe alternative, never overwrite.

### Terminology

The product must not use "Nuke" in user-facing UI. Use "Trash Now" for OS trash behavior. Language should sound calm, precise, and reversible. Avoid: Destroy, Purge, Erase forever, Clean automatically, Delete silently.

---

## 8. Audit Ledger and Undo

### Goals

The audit ledger records all file-changing actions for at least 30 days. It supports user trust, undo, crash recovery, analytics, and debugging.

### Write-ahead auditing

Every file-changing action writes an audit intent before touching the file. After the filesystem step, the tracked-file update and final audit status are committed in one database transaction. A failed filesystem step marks the intent as failed; interruption leaves a durable audit record instead of an unlogged file change.

### Undo limitations

Undo is best-effort. The app must be honest when undo is unavailable. Examples: file was moved again outside shelflife, file no longer exists, trash location unresolvable, name collision prevents restore, permission denied.

---

## 9. Storage and Runtime

### Runtime gate

Full reconciliation, watcher-driven incremental reconciliation, and automatic rule execution share one exclusive runtime gate. Their snapshot-and-write phases must not overlap, preventing a rule action from moving a file while reconciliation is persisting stale tracked-file state.

### Storage ownership

Storage owns all database opening, schema migrations, table definitions, and CRUD. Runtime lifecycle state does not live in storage. Internal records are fully normalized into SQLite columns. SQLite columns are the source of truth; storage must not duplicate full model payloads as JSON or binary blobs.

---

## 10. Path Validation

Every command that receives a path must validate:

- Path exists for file actions (undo may refer to an audit path not currently present).
- Canonical source paths are inside a configured watch target.
- Path is not a system directory or outside the user's approved scope.
- Symlinks do not escape allowed roots unless explicitly permitted.

---

## 11. Security

The frontend receives minimal permissions. Rust validates all filesystem operations. The dropzone capability has minimal command access. Commands enforce scopes internally, not relying on frontend behavior alone.

The app should block or warn before acting on: home directory root, system directories, application directories, cloud sync roots, source code repositories, external drives, and hidden files (unless approved).

---

## 12. Notifications

Notifications should be sparse and useful. Allowed notifications: files ready for review, rule preview found affected files, action completed, action failed, undo needs attention. Avoid notifications for every file event.

---

## 13. Dropzone

The dropzone is an optional transparent desktop overlay in its own always-on-top Tauri window at `/dropzone`. When enabled, dragging files and shaking the cursor shows the dropzone near the cursor. Dropped files are previewed, grouped by effective rule, and can be moved into a watch target or processed through executable rule groups. PreviewOnly rule groups are shown for explanation but cannot change files.

---

## 14. Configuration Defaults

| Setting                      | Default  |
| ---------------------------- | -------- |
| `default_ttl_seconds`        | 30 days  |
| `stale_threshold_seconds`    | 5 days   |
| `decaying_threshold_seconds` | 24 hours |
| `notifications_enabled`      | true     |
| `start_at_login`             | false    |
| `close_behavior`             | Ask      |
| `dropzone_enabled`           | true     |

---

## 15. Privacy

shelflife operates locally. It must not upload file names, paths, contents, origin URLs, or usage analytics. Any future telemetry must be explicit opt-in.

---

## 16. Implementation Status

### v1: Implemented

App watches folders, indexes files, classifies decay states, explains actions, performs reversible triage, logs all changes, validates paths in Rust, keeps frontend permissions narrow.

### v2: In Progress

- **Implemented**: dropzone window, shake monitor, preview grouping, watch-target ingest, rule-group execution, CPU-aware periodic reconciliation (defers above 70%, retries after 5 min).

### v2 acceptance criteria

- Transparent dropzone window displays and correctly ingests dragged files.
- Background scans respect resource limits (>70% CPU throttles engine).
- Code builds and tests on Windows 11.
