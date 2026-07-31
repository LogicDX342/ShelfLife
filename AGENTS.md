## Workspace rules

- Treat the checked-in codebase as the source of truth for implementation work. `SPEC.md` is the product/technical target; when it disagrees with code, verify the code first and update docs or implementation deliberately.

## Project layout

**File creation policy:** Create new files in the module that owns the behavior. Do not append unrelated code to an existing file just because it already exists. If a feature crosses seams, keep Tauri/window code in command/runtime/dropzone modules, pure engine behavior in `engine/` or `rules/`, persistence in `storage/`, and shared structs in `models/`.

### Backend (Rust) — `src-tauri/src/`

```text
src-tauri/src/
├── main.rs                  # Windows entry point only — do not add logic here
├── lib.rs                   # Tauri builder setup, plugin init, command registration, mod declarations
├── dropzone.rs              # Tauri/window-specific desktop dropzone monitor and positioning
├── tray.rs                  # System tray menu setup and window lifecycle handling
├── commands/                # Tauri IPC command handlers (one file per domain)
│   ├── mod.rs               # Re-exports all command functions
│   ├── config.rs            # config, close behavior, watch pause/resume, reconciliation scan
│   ├── dropzone.rs          # dropzone preview/ingest/rule-group/hide commands
│   ├── external.rs          # external URL opening
│   ├── files.rs             # active files, rule explanations, previews, file location, directory picker
│   ├── rules.rs             # list/save/test/delete automation rules
│   ├── tray.rs              # tray label updates
│   ├── triage.rs            # single/bulk triage, undo, audit listing, notifications
│   └── updates.rs           # app update check/install
├── runtime/                 # Tauri lifecycle orchestration and background workers
│   ├── mod.rs               # AppRuntime, setup(), watcher/dropzone sync, pause/resume
│   ├── mock.rs              # debug mock data generation
│   ├── reconciliation.rs    # Async/manual/periodic reconciliation orchestration and events
│   ├── resource_limits.rs   # CPU usage limits for background tasks
│   └── rule_scheduler.rs    # Async/periodic automatic rule execution scheduling and events
├── engine/                  # File hygiene engine (no Tauri dependency)
│   ├── mod.rs               # Re-exports
│   ├── dropzone.rs          # Dropzone preview planning and shake detection logic
│   ├── executor.rs          # Safe action execution, dropzone ingest/action, undo, rename/collision handling
│   ├── freshness.rs         # freshness_at calculation, decay state transitions, origin evidence
│   ├── paths.rs             # PathScope, config path validation, watch/safe-folder scope checks
│   ├── quiescence.rs        # Transient/system/hidden path checks and file stability checks
│   ├── reconciliation.rs    # Full and incremental watched-path reconciliation
│   ├── rule_execution.rs    # Expired automatic rule execution and failure audit entries
│   ├── rule_projection.rs   # Tracked-file rule projection and automatic-rule candidate computation
│   ├── rule_refresh.rs      # Recompute tracked files after rule changes
│   └── watcher.rs           # notify watcher setup, debounced stable-path emission
├── rules/                   # Rule engine (no Tauri dependency)
│   ├── mod.rs               # Re-exports
│   ├── conditions.rs        # Extension, glob, regex, size, origin matching
│   ├── explanation.rs       # RuleMatchExplanation generation
│   ├── rule_set.rs          # CompiledRuleSet, decide_file, RuleDecision/RuleVerdict
│   └── validation.rs        # Rule validation, rename template validation
├── storage/                 # SQLite/Diesel persistence layer
│   ├── mod.rs               # Database init, DDL bootstrap, config persistence
│   ├── audit.rs             # AuditEntry CRUD and sequence management
│   ├── migrations.rs        # Schema migrations
│   ├── rules.rs             # AutomationRule CRUD
│   ├── schema.rs            # Diesel table! metadata mirroring SCHEMA_SQL
│   ├── test_util.rs         # Rust test fixtures
│   └── tracked.rs           # TrackedFile CRUD and tracked secondary indexes
└── models/                  # Shared data types (serde structs/enums; no business logic)
    ├── mod.rs               # Re-exports all model types
    ├── audit.rs             # AuditEntry, AuditActionKind, UndoStatus, bulk triage models
    ├── config.rs            # AppConfig, WatchTarget, CloseBehavior
    ├── dropzone.rs          # Dropzone preview/action result models
    ├── error.rs             # AppError and Diesel/io conversions
    ├── rule.rs              # AutomationRule, RuleMode, RuleAction, RuleConditions
    ├── runtime.rs           # ReconciliationReport
    └── tracked_file.rs      # TrackedFile, FileDecayState, Expiry
```

**Rules:**

- `main.rs` only calls `shelflife_lib::run()`. Never modify it unless the binary entry point itself changes.
- `lib.rs` stays limited to module declarations, Tauri plugin setup, window event hooks, command registration, and `run()`. Do not add business logic there.
- `commands/` files are thin Tauri seams: validate input, call `engine/`, `rules/`, `storage/`, or `runtime/`, emit events/notifications, and return results.
- `runtime/` owns lifecycle state and orchestration: watcher restart/pause/resume, dropzone monitor sync, reconciliation scheduling, automatic rule scheduling, and runtime event emission.
- `dropzone.rs` at the backend root is Tauri/window-specific. Pure dropzone behavior belongs in `engine/dropzone.rs`; file-changing dropzone actions belong in `engine/executor.rs`.
- `engine/` and `rules/` must not depend on Tauri types. They may depend on storage models and storage APIs where the current engine interface already does, but do not introduce Tauri handles, Diesel connections, SQLite transactions, or window/event concerns there.
- `engine::watcher` must not open storage or hold database handles. It debounces events, waits for stable paths, and emits paths for `runtime/` to reconcile.
- `storage/` owns all SQLite/Diesel connections, transactions, table definitions, and Diesel `schema.rs` metadata. Other modules must not open SQLite/Diesel connections or transactions directly.
- Keep `SCHEMA_SQL` in `storage/mod.rs` and Diesel `table!` metadata in `storage/schema.rs` synchronized whenever schema changes.
- Keep tracked-file secondary index invariants inside `storage/`; expose coherent storage operations instead of making callers coordinate index updates manually.
- `models/` contains data structures with serde derives plus basic defaults/conversions only. No workflow logic.
- Automatic rule failures are stored as audit entries. Do not reintroduce in-memory retry/backoff state unless `SPEC.md` is updated first.

### Frontend (Svelte 5) — `src/`

```text
src/
├── app.html                      # HTML shell — do not modify unless changing <head>
├── app.css                       # Global styles, CSS custom properties, reset, route body variants
├── routes/                       # SvelteKit file-based routing
│   ├── +layout.svelte            # Root layout, title bar/sidebar/providers, close behavior dialog
│   ├── +layout.ts                # SSR disabled (SPA mode)
│   ├── +page.svelte              # Dashboard route
│   ├── about/+page.svelte        # About page route
│   ├── audit/+page.svelte        # Audit log route
│   ├── browser/+page.svelte      # File browser route
│   ├── dropzone/+page.svelte     # Dropzone window route
│   ├── queue/+page.svelte        # Review queue route
│   ├── rules/+page.svelte        # Rule editor/list route
│   └── settings/+page.svelte     # Watch targets, preferences, config route
└── lib/
    ├── api/                      # Typed Tauri invoke wrappers only
    │   ├── config.ts
    │   ├── dropzone.ts
    │   ├── external.ts
    │   ├── files.ts
    │   ├── rules.ts
    │   ├── tray.ts
    │   ├── triage.ts
    │   └── updates.ts
    ├── components/               # Product UI modules
    │   ├── common/               # Shared shell states: PageHeader, PageBody, EmptyState, LoadingState
    │   ├── ui/                   # shadcn-svelte primitives; keep product logic out of these files
    │   ├── AboutView.svelte
    │   ├── AuditRow.svelte
    │   ├── AuditView.svelte
    │   ├── ConfirmDialog.svelte
    │   ├── DashboardStatCard.svelte
    │   ├── DashboardView.svelte
    │   ├── DecayTimelineSlider.svelte
    │   ├── DropzoneView.svelte
    │   ├── ExplanationBadge.svelte
    │   ├── FileBrowser.svelte
    │   ├── FileCard.svelte
    │   ├── PriorityStack.svelte
    │   ├── RuleCard.svelte
    │   ├── RuleEditor.svelte
    │   ├── RuleList.svelte
    │   ├── RuleTestResults.svelte
    │   ├── RulesView.svelte
    │   ├── SettingsView.svelte
    │   ├── Sidebar.svelte
    │   ├── StatusBar.svelte
    │   ├── TitleBar.svelte
    │   └── WatchTargetCard.svelte
    ├── i18n/                     # Internationalization
    │   ├── i18n.svelte.ts        # Central translation registry and language/theme state
    │   └── locales/              # Locale-specific translation files
    ├── live/liveSnapshots.ts     # Owner of backend event names, coalesced refresh, focus refresh
    ├── rules/                    # Frontend rule template definitions
    │   └── templates.ts          # Starter rule catalog and rule construction
    ├── stores/                   # Svelte 5 rune state classes
    │   ├── audit.svelte.ts
    │   ├── files.svelte.ts
    │   ├── notifications.svelte.ts
    │   └── rules.svelte.ts
    ├── types/index.ts            # TypeScript mirrors of Rust IPC models
    ├── utils.ts                  # shadcn/classname helpers
    └── utils/
        ├── format.ts             # File size/date/error formatting
        └── moveDestinations.ts   # Recent move destination persistence and deduplication
```

**Rules:**

- Route `+page.svelte` files are page-level composition only. They should import view components and stay small.
- Reusable/product UI belongs in `src/lib/components/`. Shared primitives generated from shadcn-svelte live under `src/lib/components/ui/`; avoid product-specific state or commands there.
- All Tauri IPC calls go through `src/lib/api/`. Components and stores must not call `invoke()` directly.
- `src/lib/live/liveSnapshots.ts` owns live backend event names for files, audit entries, and reconciliation progress. Views should not register duplicate file/audit/reconciliation listeners directly.
- All TypeScript types mirroring Rust structs go in `src/lib/types/index.ts`.
- Use Svelte 5 runes exclusively: `$state`, `$derived`, `$effect`. Do not use Svelte 4 `$:` reactive statements or `$` store subscriptions.
- Stores use `.svelte.ts` extension for rune support.
- User-facing strings belong in `src/lib/i18n/i18n.svelte.ts`.

### Config files (root)

```text
ShelfLife/
├── package.json              # Frontend deps/scripts (pnpm)
├── pnpm-lock.yaml            # Lockfile
├── pnpm-workspace.yaml       # Workspace definition
├── components.json           # shadcn-svelte registry/config
├── eslint.config.js          # ESLint config
├── svelte.config.js          # SvelteKit adapter-static (SPA mode for Tauri)
├── vite.config.js            # Vite dev server (port 1420, ignores src-tauri/)
├── tsconfig.json             # TypeScript config
├── static/
│   └── favicon.png
└── src-tauri/
    ├── Cargo.toml            # Rust dependencies
    ├── Cargo.lock            # Rust lockfile
    ├── tauri.conf.json       # Tauri app config, main/dropzone windows, bundle
    ├── build.rs              # Tauri build script
    ├── capabilities/
    │   ├── default.json      # IPC permissions for main window
    │   └── dropzone.json     # Minimal IPC permissions for dropzone window
    └── icons/                # App icons (all sizes + .ico)
```

---

## Dev environment tips

- Run `cargo tauri dev` from the root directory to spin up the Rust backend daemon and the Svelte 5 HMR frontend simultaneously.
- Run `cargo add <crate_name> --manifest-path src-tauri/Cargo.toml` to add pure-Rust dependencies directly to the backend layer.
- Use `pnpm add -D <package_name>` at the root to add frontend utilities, Tailwind extensions, or UI plugins so Vite and Svelte can index them.
- Check `src-tauri/Cargo.toml` for backend feature flags and `src/` for Svelte 5 application views.
- When working in Svelte components, exclusively use Svelte 5 runes (`$state`, `$derived`, `$effect`). Do not use legacy Svelte 4 `$` stores or `$` reactive assignments.
- Do not add `#[allow(dead_code)]` annotations to bypass compiler warnings or lints. Fix the code instead.
- Do not run lint or check commands such as `pnpm verify`; husky will run them automatically.
- Do not stage changes; the user will review and commit them.

### Windows-specific notes

- The target platform for v1/v2 work in this repo is **Windows only**. Do not add macOS- or Linux-specific behavior unless gated behind `#[cfg(target_os)]`.
- Long paths (> 260 chars) may fail on older Windows configurations — use canonicalization carefully and preserve user-facing original paths where audit/debug output needs them.
- Zone.Identifier alternate data streams are read via `<filepath>:Zone.Identifier:$DATA`. Test with files downloaded through Edge/Chrome when changing origin evidence.
- Dropzone cursor monitoring uses Windows APIs through `windows-sys`; keep platform-specific code gated.

---

## Testing & validation instructions

- **Test coverage:** Only write tests for critical business logic. Do not test boilerplate.
- **Strict validation:** Fix compiler warnings, clippy lints, and frontend type mismatches relevant to the files you changed.
- **Backend commands (Tauri):** When modifying Rust code, run `cargo test --manifest-path src-tauri/Cargo.toml`.
- Respect the repo instruction not to run broad frontend lint/check commands unless explicitly requested.

---

## PR & commit instructions

- **Commit style:** Strictly use conventional commits with a scope when applicable, e.g. `feat(dropzone):`, `fix(storage):`, `chore(docs):`.
