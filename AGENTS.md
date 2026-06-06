## Project layout

```text
ShelfLife/
├── SPEC.md                       # Product and technical specification
├── AGENTS.md                     # This file — dev environment and conventions
├── package.json                  # Frontend dependencies (pnpm)
├── pnpm-lock.yaml                # Lockfile
├── svelte.config.js              # SvelteKit adapter-static config for Tauri SPA
├── vite.config.js                # Vite dev server config (port 1420)
├── tsconfig.json                 # TypeScript config
├── static/                       # Static assets served at /
│   └── favicon.png
├── src/                          # Svelte 5 frontend
│   ├── app.html                  # HTML shell
│   └── routes/                   # SvelteKit file-based routes
│       ├── +layout.ts            # SSR disabled (SPA mode)
│       └── +page.svelte          # Main dashboard page
└── src-tauri/                    # Rust backend (Tauri v2)
    ├── Cargo.toml                # Rust dependencies
    ├── tauri.conf.json           # Tauri app config (window, tray, bundle)
    ├── capabilities/
    │   └── default.json          # IPC permissions for main window
    ├── build.rs                  # Tauri build script
    ├── icons/                    # App icons (all sizes + .ico)
    └── src/
        ├── main.rs               # Windows entry point (hides console in release)
        └── lib.rs                # App bootstrap: Tauri builder, plugin init, command registration
```

### Key entry points

| Layer    | File                            | Purpose                                              |
|----------|---------------------------------|------------------------------------------------------|
| Frontend | `src/routes/+page.svelte`       | Main dashboard view                                  |
| Frontend | `src/app.html`                  | HTML shell for SvelteKit                             |
| Backend  | `src-tauri/src/lib.rs`          | Tauri app builder, plugin registration, IPC handlers |
| Backend  | `src-tauri/src/main.rs`         | Windows process entry point                          |
| Config   | `src-tauri/tauri.conf.json`     | Window size, tray icon, CSP, bundle targets          |
| Config   | `src-tauri/capabilities/*.json` | Per-window IPC permission grants                     |

### Where to add new things

* **New Tauri command**: Define in `src-tauri/src/lib.rs` (or a module imported there), register in `tauri::generate_handler![]`.
* **New Rust module**: Create `src-tauri/src/<module>.rs`, add `mod <module>;` to `lib.rs`.
* **New redb table**: Define `TableDefinition` constant alongside existing ones, create table in the database init function.
* **New Svelte route**: Add `src/routes/<name>/+page.svelte`.
* **New frontend component**: Add to `src/lib/components/<Name>.svelte`.
* **New static asset**: Place in `static/`.

---

## Dev environment tips

* Run `cargo tauri dev` from the root directory to spin up the Rust backend daemon and the Svelte 5 HMR frontend simultaneously.
* Run `cargo add <crate_name> --manifest-path src-tauri/Cargo.toml` to add pure-Rust dependencies (like `redb` or `notify`) directly to the backend layer.
* Use `pnpm add -D <package_name>` at the root to add frontend utilities, Tailwind extensions, or UI plugins so Vite and Svelte can index them.
* Check `src-tauri/Cargo.toml` to manage backend feature flags and `src/` for Svelte 5 application views.
* When working in Svelte components, exclusively use Svelte 5 runes (`$state`, `$derived`, `$effect`). Do not use legacy Svelte 4 `$` stores or `$` reactive assignments.

### Windows-specific notes

* The target platform for v1 is **Windows only**. Do not add macOS- or Linux-specific code paths unless gated behind `#[cfg(target_os)]`.
* Long paths (> 260 chars) may fail on older Windows configurations — use `\\?\` prefix or `std::fs::canonicalize` when handling user paths.
* Zone.Identifier alternate data streams are read via `<filepath>:Zone.Identifier:$DATA`. Test with files downloaded through Edge/Chrome.
* The Recycle Bin integration uses the `trash` crate which calls `IFileOperation` on Windows.

---

## Testing instructions

* Run `cargo test --manifest-path src-tauri/Cargo.toml` to run all native Rust unit tests for the `redb` storage engines, file age calculations, and regex whitelist filters.
* Run `pnpm test` to execute frontend tests (when configured).
* Run `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` before committing to catch any non-idiomatic Rust, potential thread blocks, or unhandled Result types.
* Run `pnpm lint && pnpm check` to validate that ESLint, Prettier, and Svelte-Check pass without warnings across all `.svelte` and `.ts` files.
* To test platform-specific file watching locally, create a temporary directory (e.g., `test-watch/`) in the project root, configure it as a watch target, then drop sample files into it and monitor terminal logs from the `notify` watcher thread.
* Fix all compiler warnings, clippy lints, and frontend type mismatches until the entire validation pipeline turns green.

---

## PR instructions

* Use conventional commits.
* Always run `cargo clippy`, `pnpm lint`, and a test execution pass before staging commits.
* Run a trial production compile using `cargo tauri build --no-bundle` to guarantee that the application compiles under strict release profiling before opening a Pull Request.