# 0025: Separate SQLite database and keychain entry for dev vs. prod builds, and label the dev window

**Status:** Accepted — 2026-08-24

## Context

The SQLite path in `lib.rs` was `app_data_dir/timetracking.sqlite3` regardless of how
the app was launched, and the keychain `SERVICE_NAME` in `secrets/keyring_store.rs` was
a single fixed string. Both `app_data_dir` and the keychain service are namespaced by
`tauri.conf.json`'s `identifier`, which is the same for `npm run tauri dev` and
`npm run tauri build` — so a debug run and a release build were reading and writing the
exact same database file and the exact same keychain entry. Timers/entries created
while developing landed in the same place as real tracked time, and saving Jira
settings in one build silently overwrote the other's.

## Decision

Key both the SQLite filename and the keychain `SERVICE_NAME` off `cfg!(debug_assertions)`,
which is `true` for `tauri dev` (debug builds) and `false` for `tauri build` (release
builds):

- `lib.rs`: db file is `timetracking-dev.sqlite3` in debug builds, `timetracking.sqlite3`
  in release builds (same `app_data_dir` — the identifier is unchanged).
- `secrets/keyring_store.rs`: `SERVICE_NAME` is `com.georg.timetracking.dev` in debug
  builds, `com.georg.timetracking` in release builds.
- `lib.rs::setup`: in debug builds, the main window title gets a `" - DEV"` suffix
  appended at startup (via `WebviewWindow::set_title`, since `tauri.conf.json`'s
  static `title` has no per-profile variant) — otherwise a dev and a prod window look
  identical side by side, which invites acting on the wrong one.

This piggybacks on a distinction Tauri already makes for every dev/build invocation, so
no new env var, CLI flag, or build profile is needed, and there's no risk of forgetting
to set one before running `tauri dev`.

The keychain is split alongside the database, not just the database: settings rows
(`jira_base_url`/`jira_email`) and the keychain token are meant to describe the same
Jira connection. Splitting only the database would leave both builds sharing one
keychain entry, so saving a (possibly throwaway/test) token in a dev build would
silently replace the token the release build authenticates with, and vice versa.

## Consequences

- A dev build and a release build now behave like fully independent app installs: each
  needs its own Setup flow the first time, and it's safe to test destructive flows
  (sync, delete, keychain clear) in a dev build without touching real tracked time or
  the real Jira token.
- Existing users upgrading a release build see no change — the release path/service
  name are unchanged, so their existing db/keychain entry is picked up as before. Any
  data previously written by a debug build under the old shared filename/service name
  is orphaned (not migrated); this is acceptable since debug-build data was never meant
  to be authoritative.
- Anyone building with `cargo build`/`cargo test` directly (not via the Tauri CLI) also
  gets the dev paths, since `debug_assertions` reflects the Cargo profile, not
  specifically the `tauri` CLI subcommand.

## Alternatives considered

- **A dedicated env var (e.g. `APP_ENV=dev`)** — more explicit, but adds a step that's
  easy to forget before `tauri dev`/`tauri build`; `debug_assertions` already tracks
  this distinction with zero extra configuration.
- **Separate `identifier` per environment in `tauri.conf.json`** — would also separate
  `app_data_dir` entirely (cleanest OS-level isolation), but `tauri.conf.json` isn't
  currently profile-aware in this project, and would require maintaining two config
  files or a build-time templating step for one identifier string; deferred as
  unnecessary given `debug_assertions` already solves the stated problem.
