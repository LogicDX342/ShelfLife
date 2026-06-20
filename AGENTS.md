## Workspace rules

- Default to `rtk` for every shell command except powershell command, and reference `.agents/rules/RTK.md`.

## Project layout

**File creation policy:** You are expected to create new files as needed to follow this structure. The directory skeleton exists with stub `mod.rs` and `index.ts` files. When implementing a feature, create the appropriate file in the correct module — do not append unrelated code to an existing file just because it already exists.

### Backend (Rust) — `src-tauri/src/`

```text
src-tauri/src/
├── main.rs                  # Windows entry point only — do not add logic here
├── lib.rs                   # Tauri builder setup, plugin init, mod declarations
│                            #   ONLY contains: mod statements, run() with Builder
├── commands/                # Tauri IPC command handlers (one file per domain)
│   ├── mod.rs               # Re-exports all command functions
│   ├── files.rs             # get_active_files, explain_file, preview_file
│   ├── triage.rs            # execute_triage_action, undo_audit_entry
│   ├── rules.rs             # list_rules, save_rule, test_rule, delete_rule
│   └── config.rs            # update_watch_targets, get_config, save_config
├── runtime/                 # Tauri lifecycle orchestration and background workers
│   ├── mod.rs               # AppRuntime, setup(), sync_after_config_change()
│   ├── reconciliation.rs    # Async/manual/periodic reconciliation orchestration
│   └── rule_scheduler.rs    # Async/periodic automatic rule execution scheduling
├── engine/                  # File hygiene engine (no Tauri dependency)
│   ├── mod.rs               # Re-exports
│   ├── watcher.rs           # notify watcher setup, debounced stable-path emission
│   ├── quiescence.rs        # File stability checks (size + mtime confirmation)
│   ├── reconciliation.rs    # Startup and periodic full-directory scans
│   ├── freshness.rs         # freshness_at calculation, decay state transitions
│   └── executor.rs          # Safe action execution (move, rename, trash)
├── rules/                   # Rule engine (no Tauri dependency)
│   ├── mod.rs               # Re-exports
│   ├── evaluator.rs         # Rule matching and priority ordering
│   ├── conditions.rs        # Extension, glob, regex, size, origin matching
│   └── explanation.rs       # RuleMatchExplanation generation
├── storage/                 # redb persistence layer
│   ├── mod.rs               # Re-exports, Database init, table definitions
│   ├── tracked.rs           # CRUD for TrackedFile records
│   ├── rules.rs             # CRUD for AutomationRule records
│   └── audit.rs             # CRUD for AuditEntry records, sequence management
├── models/                  # Shared data types (no logic, just structs/enums)
│   ├── mod.rs               # Re-exports all types
│   ├── tracked_file.rs      # TrackedFile, FileDecayState, Expiry
│   ├── rule.rs              # AutomationRule, RuleMode, RuleAction, RuleConditions
│   ├── audit.rs             # AuditEntry, AuditActionKind, UndoStatus
│   ├── origin.rs            # OriginEvidence
│   ├── config.rs            # AppConfig, WatchTarget
│   ├── preview.rs           # FilePreview, FilePreviewContent
│   └── error.rs             # AppError, error codes
└── tray.rs                  # System tray menu setup and event handling
```

**Rules:**

- `lib.rs` only contains `mod` declarations and the `run()` function that builds the Tauri app. No business logic.
- `main.rs` only calls `shelflife_lib::run()`. Never modify it.
- `commands/` files are thin wrappers — they validate input, call into `engine/`, `storage/`, or `runtime/`, and return results.
- `runtime/` owns lifecycle state and orchestration: watcher restart/pause/resume, dropzone monitor sync, reconciliation scheduling, automatic rule execution scheduling, and runtime event emission.
- `runtime/mod.rs` should stay small: `AppRuntime`, `setup()`, and `sync_after_config_change()`. Put reconciliation orchestration in `runtime/reconciliation.rs` and rule scheduling in `runtime/rule_scheduler.rs`.
- `engine/` and `rules/` must NOT depend on Tauri types. They are pure Rust libraries testable without Tauri.
- `engine::watcher` must not open storage or hold database handles. It debounces events, waits for stable paths, and emits paths for `runtime/` to reconcile.
- `models/` contains only data structures with `serde` derives. No methods beyond basic constructors.
- `storage/` owns all redb access. Other modules never open database transactions directly, and runtime lifecycle state must not live in `storage/`.
- Automatic rule failures are stored as audit entries. Do not reintroduce in-memory retry/backoff state unless the product spec is updated first.

### Frontend (Svelte 5) — `src/`

```text
src/
├── app.html                      # HTML shell — do not modify unless changing <head>
├── app.css                       # Global styles, CSS custom properties, reset
├── routes/                       # SvelteKit file-based routing
│   ├── +layout.svelte            # Root layout: sidebar/nav, global providers
│   ├── +layout.ts                # SSR disabled (SPA mode)
│   ├── +page.svelte              # Dashboard (main landing page only)
│   ├── rules/
│   │   └── +page.svelte          # Rule editor / rule list page
│   ├── audit/
│   │   └── +page.svelte          # Audit log / undo page
│   └── settings/
│       └── +page.svelte          # Watch targets, preferences, config
└── lib/                          # Shared code (NOT route pages)
    ├── components/               # Reusable Svelte 5 components
    │   ├── FileCard.svelte       # Single file card (decay state, actions)
    │   ├── FileList.svelte       # Grouped file list with filters
    │   ├── RuleEditor.svelte     # Single rule create/edit form
    │   ├── AuditRow.svelte       # Single audit entry with undo button
    │   ├── PreviewPanel.svelte   # Lazy file preview (text/image/pdf/unknown)
    │   ├── ExplanationBadge.svelte  # Rule match explanation tooltip/card
    │   ├── ConfirmDialog.svelte  # Action confirmation modal
    │   └── StatusBar.svelte      # Watch status, file counts summary
    ├── stores/                   # Svelte 5 reactive state ($state wrappers)
    │   ├── files.svelte.ts       # Tracked file list state, refresh logic
    │   ├── rules.svelte.ts       # Rule list state
    │   └── audit.svelte.ts       # Audit entries state
    ├── api/                      # Tauri IPC call wrappers (typed invoke calls)
    │   ├── files.ts              # invoke("get_active_files"), invoke("explain_file"), etc.
    │   ├── triage.ts             # invoke("execute_triage_action"), invoke("undo_audit_entry")
    │   ├── rules.ts              # invoke("list_rules"), invoke("save_rule"), etc.
    │   └── config.ts             # invoke("update_watch_targets"), invoke("get_config")
    ├── types/                    # TypeScript type definitions mirroring Rust models
    │   └── index.ts              # TrackedFile, AutomationRule, AuditEntry, etc.
    └── utils/                    # Pure helper functions
        ├── format.ts             # File size formatting, date display
        └── decay.ts              # Decay state label/color derivation
```

**Rules:**

- Route `+page.svelte` files are page-level composition only — they import components and wire up data. Keep them under ~100 lines.
- All reusable UI goes in `src/lib/components/`. If a piece of UI appears in more than one page, extract it.
- All Tauri IPC calls go through `src/lib/api/`. Components never call `invoke()` directly.
- All TypeScript types mirroring Rust structs go in `src/lib/types/`.
- Use Svelte 5 runes exclusively: `$state`, `$derived`, `$effect`. Do NOT use Svelte 4 `$:` reactive statements or `$` store subscriptions.
- Stores use `.svelte.ts` extension for rune support.

### Config files (root)

```text
ShelfLife/
├── package.json              # Frontend deps (pnpm)
├── pnpm-lock.yaml            # Lockfile
├── svelte.config.js          # SvelteKit adapter-static (SPA mode for Tauri)
├── vite.config.js            # Vite dev server (port 1420, ignores src-tauri/)
├── tsconfig.json             # TypeScript config
├── static/                   # Static assets served at /
│   └── favicon.png
└── src-tauri/
    ├── Cargo.toml            # Rust dependencies
    ├── tauri.conf.json       # Tauri app config (window, tray, bundle)
    ├── build.rs              # Tauri build script
    ├── capabilities/
    │   └── default.json      # IPC permissions for main window
    └── icons/                # App icons (all sizes + .ico)
```

---

## Dev environment tips

- Run `cargo tauri dev` from the root directory to spin up the Rust backend daemon and the Svelte 5 HMR frontend simultaneously.
- Run `cargo add <crate_name> --manifest-path src-tauri/Cargo.toml` to add pure-Rust dependencies (like `redb` or `notify`) directly to the backend layer.
- Use `pnpm add -D <package_name>` at the root to add frontend utilities, Tailwind extensions, or UI plugins so Vite and Svelte can index them.
- Check `src-tauri/Cargo.toml` to manage backend feature flags and `src/` for Svelte 5 application views.
- When working in Svelte components, exclusively use Svelte 5 runes (`$state`, `$derived`, `$effect`). Do not use legacy Svelte 4 `$` stores or `$` reactive assignments.
- Do not add `#[allow(dead_code)]` annotations to code to bypass compiler warnings or lints. Fix the code instead.
- Do not run any lint or check commands such as `pnpm verify`, as husky will run them automatically.

### Windows-specific notes

- The target platform for v1 is **Windows only**. Do not add macOS- or Linux-specific code paths unless gated behind `#[cfg(target_os)]`.
- Long paths (> 260 chars) may fail on older Windows configurations — use `\\?\` prefix or `std::fs::canonicalize` when handling user paths.
- Zone.Identifier alternate data streams are read via `<filepath>:Zone.Identifier:$DATA`. Test with files downloaded through Edge/Chrome.

---

## Testing & Validation Instructions

- **Test Coverage:** Only write tests for critical business logic. Do not test boilerplate.
- **Strict Validation:** Fix all compiler warnings, clippy lints, and frontend type mismatches until the relevant validation commands are green.
- **Backend Commands (Tauri):** When modifying Rust code, run `cargo test --manifest-path src-tauri/Cargo.toml`

---

## PR & Commit Instructions

- **Commit Style:** Strictly use conventional commits (e.g., `feat:`, `fix:`, `chore:`).
