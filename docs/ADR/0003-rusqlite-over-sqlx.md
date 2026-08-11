# 0003: Use `rusqlite` instead of `sqlx` for local storage

**Status:** Accepted — 2026-08-10

## Context

The spec allows either `rusqlite` or `sqlx` for the embedded SQLite database. This is
a single-user desktop app: one process, one local `.sqlite3` file, no concurrent
remote clients, and query volume is trivial (a handful of rows per day of usage).
`sqlx` is async-native with compile-time-checked queries (via a `DATABASE_URL`/offline
`.sqlx` metadata setup), while `rusqlite` is a synchronous, direct binding to SQLite's
C API.

## Decision

Use **`rusqlite`** (with the `bundled` feature, to avoid depending on the host's
system `libsqlite3` version) plus **`rusqlite_migration`** for schema migrations. The
connection is wrapped in `Mutex<Connection>` inside `AppState` and accessed
synchronously from within `#[tauri::command]` functions — Tauri commands may be
`async fn` or plain `fn` freely, and DB operations here are microsecond-scale, so
there's no need to route them through a thread pool or async driver.

## Consequences

- No `DATABASE_URL` / `cargo sqlx prepare` build-time ceremony — migrations
  (`src-tauri/migrations/001_initial.sql`) are applied at runtime via
  `db::connection::open_app_db`, and schema changes only require adding a new
  migration file.
- All DB access funnels through a single `Mutex<Connection>`; a long-held lock across
  an `.await` would block every other command. This is why `sync::service::sync_all`
  explicitly re-locks around each DB read/write and never holds the guard across the
  network `.await` to Jira (see ADR-0008).
- If the app ever needed concurrent writers, connection pooling, or async query
  execution, this decision would need revisiting — none of those apply at the current
  single-process, single-connection scale.

## Alternatives considered

- **`sqlx`** — rejected for the added async/compile-time-query setup complexity with
  no corresponding benefit at this app's scale and concurrency profile.
