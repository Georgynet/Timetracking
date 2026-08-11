# 0001: Use Tauri v2 as the application shell

**Status:** Accepted — 2026-08-10

## Context

The spec mandates a desktop app with a Rust backend and a web frontend, targeting
macOS as the primary platform and Ubuntu Linux as a secondary one, with a native
system tray where feasible. Tauri is named explicitly in the spec. At the time of
implementation, Tauri has two active major lines: v1 (legacy, in maintenance mode) and
v2 (current, with a redesigned plugin/permission system and a unified tray-icon API
shared across desktop platforms).

## Decision

Use **Tauri v2**, scaffolded via `npm create tauri-app@latest` with its React +
TypeScript template. The tray icon and menu are built with Tauri v2's core
`tray-icon` feature (not a separate plugin), and window/webview access from the
frontend goes through the v2 `@tauri-apps/api` package.

## Consequences

- Gets the maintained plugin/capability system and tray API; v1-era examples and
  some third-party plugins found online will not directly apply.
- The `capabilities/*.json` permission files gate plugin-provided and core
  window/webview APIs exposed to the frontend, but not our own
  `#[tauri::command]` functions registered via `generate_handler!` — those are
  always invokable once registered, so no capability entries were needed beyond the
  scaffold defaults for this app (no fs/shell/dialog plugins are used).
- `rusqlite`'s `bundled` SQLite feature is used specifically to avoid depending on
  the system `libsqlite3`, whose version can otherwise vary between macOS and Ubuntu
  and between Tauri v1/v2-era webview runtimes.

## Alternatives considered

- **Tauri v1** — rejected: legacy line, no reason to start a new project on it.
- **Electron** — not considered seriously: the spec explicitly calls for a Rust
  backend and Tauri; Electron would mean a Node backend and a much heavier bundle.
