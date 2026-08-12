# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Desktop time-tracking app (Tauri v2 + React/TypeScript frontend, Rust backend) that tracks time
against Jira tickets and manually syncs worklogs to Jira Cloud. Full functional spec is in
`spec_time_tracker_jira.md` — read it before implementing new features, since it defines the data
model and behavioral rules (single active timer, records are never deleted, sync is manual-only,
update-vs-create worklog semantics, etc.) that the code below is deliberately built to satisfy.

## Commands

Frontend (run from repo root):
```sh
npm install
npm run tauri dev      # full app: Rust backend + Vite frontend with HMR
npm run dev             # frontend only (vite), no Tauri window
npm run build            # tsc typecheck + vite build
npm run tauri build      # full app bundle (.app/.dmg on macOS, .deb/.AppImage on Linux)
```

Backend (run from `src-tauri/`):
```sh
cargo test                                          # unit tests + wiremock-based Jira client tests
cargo test <test_name>                               # run a single test by name substring
cargo test -- --ignored real_keychain_round_trip     # touches the real OS keychain; skipped by default
cargo build
```

There is no separate lint/format command configured beyond `tsc` (frontend type-checking, part of
`npm run build`) and standard `cargo build` warnings.

## Architecture Decision Records

Every feature or significant technical decision requires an ADR in `docs/ADR/`, following the
lightweight Nygard-style format (Context, Decision, Consequences, Alternatives considered) already
used by the existing 16 records — see `docs/ADR/README.md` for the format and index. When
implementing a new feature: add a new `NNNN-kebab-case-title.md` file (next sequential number),
add a row to the table in `docs/ADR/README.md`, and mark it `Accepted` once the approach is
settled. Once accepted, an ADR is never edited to reflect a later change of decision — write a new
ADR that supersedes it instead, so the history of *why* stays intact.

## Architecture

### Split between frontend and backend

Almost all business logic lives in Rust (`src-tauri/src/`), invoked from React via Tauri
`invoke()` calls. The frontend is a thin view/state layer:

- `src/api/commands.ts` — one wrapper function per Tauri command, the only place that calls
  `invoke()`. `src/api/types.ts` mirrors the Rust DTOs (camelCase, per `#[serde(rename_all = "camelCase")]`
  on the Rust side).
- `src/state/store.ts` — single Zustand store; each action calls the API then re-reads
  authoritative state from the backend rather than optimistically mutating local state (e.g.
  `startTimer`/`stopTimer` always re-fetch `activeTimer` in a `finally`, even on error, so the UI
  self-corrects if its cached state had drifted).
- `src/views/*` — one component per UI section (`SetupView`, `MainView`, `TimerWidget`,
  `MyTasksPanel`, `FavoritesPanel`, `HistoryList`, `ManualEntryForm`, `SyncReportModal`,
  `HeaderBar`). `App.tsx` just branches between `SetupView` (not configured yet) and `MainView`.

### Rust backend layout (`src-tauri/src/`)

- `commands/*` — `#[tauri::command]` functions, grouped by feature (`setup`, `tasks`, `timer`,
  `entries`, `sync`). These are thin: they pull `AppState`, call into `db::*_repo` / `timer::engine`
  / `sync::service`, and map errors into `AppError`. All registered in the `invoke_handler!` list in
  `lib.rs`.
- `state.rs` — `AppState`, held by Tauri as managed state: a `Mutex<Connection>` (single shared
  SQLite connection) plus a `Mutex<Option<Arc<dyn JiraClient>>>` that is `None` until Setup
  succeeds, and an `AtomicBool` for tray availability.
- `db/` — `connection.rs` (opens + runs migrations via `rusqlite_migration`), `migrations.rs`,
  `models.rs` (row structs), and one `*_repo.rs` per table (`settings_repo`, `tasks_repo`,
  `time_entries_repo`). Repos are the only code that writes raw SQL.
- `timer/engine.rs` — enforces "only one running timer": starting a new timer stops-and-saves the
  currently running entry, both inside one DB transaction, backed by a DB-level unique partial
  index (`idx_time_entries_single_running` in `migrations/001_initial.sql`) as a second line of
  defense against races.
- `sync/service.rs` — `sync_all` pushes every `is_synced = 0` entry to Jira, oldest first. A
  worklog is created (`POST`) if `jira_worklog_id` is `NULL`, or updated (`PUT`) if a prior sync
  already produced one — this is how editing an already-synced entry re-syncs without duplicating.
  A failing entry is left completely untouched (so it's naturally retried later) and does not stop
  the rest of the batch. The DB mutex is only held around the brief read/write per entry, never
  across the network `.await`.
- `jira/` — `client_trait.rs` defines the `JiraClient` trait (all Jira REST calls the app needs);
  `reqwest_client.rs` is the real implementation, `fake_client.rs` is an in-memory test double used
  by `timer`/`sync` unit tests, and `tests/jira_client_tests.rs` (top-level `src-tauri/tests/`)
  exercises `reqwest_client` against a local `wiremock` server. Business logic should depend on the
  `JiraClient` trait, not on `reqwest_client` directly, to stay testable without HTTP.
- `secrets/keyring_store.rs` — the Jira API token lives *only* in the OS keychain (macOS
  Keychain / Linux Secret Service via `keyring` crate), never in the SQLite DB or in memory beyond
  what's needed to build a `JiraClient`. `lib.rs::restore_jira_client` reloads it into `AppState` on
  startup so the keychain isn't touched on every request.
- `error.rs` — a single `AppError` enum that every command returns as `Result<T, AppError>`;
  serializes to a plain string for the frontend.

### Secret-safety convention (deliberate, tested)

This codebase treats "never log or leak the Jira API token" as an invariant enforced by
construction, not by care at call sites — preserve this pattern when touching Jira/logging code:
1. The `settings` table/struct has no token field at all (only `jira_base_url`/`jira_email`).
2. `jira::models::JiraError` is only ever constructed from HTTP status codes and response bodies,
   never from request headers — see the regression test
   `jira_error_variants_never_embed_request_headers` in `jira/models.rs`.
3. Don't log a whole request/response with `{:?}`; log individual primitive fields instead (see
   `logging.rs` doc comment for the full rationale).

### Jira worklog comment format

Jira Cloud's worklog `comment` field requires Atlassian Document Format (ADF), not plain text.
`jira::models::plain_text_to_adf` converts a plain string to ADF, returning `None` for
empty/whitespace input (comment is optional per spec) — go through this helper rather than
hand-building ADF elsewhere.

### Cross-platform tray fallback

The system tray is built programmatically in `lib.rs::build_tray` (not declared in
`tauri.conf.json`) specifically so a failure (e.g. stock GNOME without an AppIndicator extension)
can be caught and the app falls back to window-only mode via `AppState::tray_available` /
`commands::is_tray_available`, instead of crashing. Preserve this fallback if touching tray setup.

### Testing conventions (Rust)

- Repo/business-logic tests use `db::connection::open_in_memory()` for a throwaway SQLite DB — no
  real file I/O.
- `timer` and `sync` tests use `jira::fake_client::FakeJiraClient` instead of real HTTP.
- Jira HTTP-layer tests use `wiremock` (`src-tauri/tests/jira_client_tests.rs`) — no live Jira
  instance is needed or used in CI.
- Tests marked `#[ignore]` touch real external systems (e.g. the OS keychain) and must be run
  explicitly, not as part of the default suite.

## Known gaps (see README.md for full detail)

Built and verified on macOS only:
- Linux tray (AppIndicator dependency) and Linux keychain (Secret Service D-Bus) are implemented
  but untested against a real environment.
- `.deb`/AppImage bundling is configured but never actually built.
- The Jira client is verified only against `wiremock`, not a live Jira Cloud instance — the
  create-vs-update worklog logic and Jira's accepted date/ADF format need a real end-to-end pass
  before being trusted.
