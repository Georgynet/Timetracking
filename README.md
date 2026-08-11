# Time Tracker with Jira Sync

A desktop time-tracking app (macOS primary, Ubuntu Linux secondary) that tracks time
against Jira tickets and manually pushes worklogs to Jira Cloud. See
`spec_time_tracker_jira.md` for the full functional spec.

Stack: Tauri v2 (Rust backend) + React/TypeScript/Vite frontend, SQLite (`rusqlite`)
for local storage, the OS keychain (`keyring` crate) for the Jira API token.

## Prerequisites

- Rust (via [rustup](https://rustup.rs/))
- Node.js 18+
- macOS: Xcode Command Line Tools (`xcode-select --install`)
- Ubuntu: Tauri's Linux system dependencies — see
  <https://tauri.app/start/prerequisites/#linux> (`libwebkit2gtk`, `libayatana-appindicator3-dev`
  for the tray icon, etc.)

## Development

```sh
npm install
npm run tauri dev
```

## Building

```sh
npm run tauri build
```

Produces a `.app`/`.dmg` on macOS and a `.deb`/`.AppImage` on Linux (bundle targets are
set to `"all"` in `src-tauri/tauri.conf.json`, so the bundler picks the right output
per host OS).

## Tests

```sh
cd src-tauri
cargo test              # unit + wiremock-based Jira HTTP client tests (no live Jira needed)
cargo test -- --ignored real_keychain_round_trip   # touches the real OS keychain
```

## Known gaps / what still needs verification

This was built and verified on macOS only. Written to be cross-platform, but not yet
run on Ubuntu:

- **Tray icon on Linux**: stock GNOME has no tray/StatusNotifierItem support without an
  extension (e.g. AppIndicator). If the tray fails to initialize, the app falls back to
  a window-only mode — the header bar shows a "tray unavailable" note and the timer
  status stays visible there instead. Needs a real check on GNOME and KDE.
- **Keychain on Linux**: uses the Secret Service D-Bus API (`zbus-secret-service-keyring-store`),
  which needs a running provider (gnome-keyring, KWallet). Untested against a real one.
- **`.deb`/`AppImage` output**: bundle config is written but never actually built/run.
- **Live Jira**: no Jira Cloud credentials were available while building this, so the
  Jira client is verified against `wiremock` (a local mock HTTP server) rather than a
  real instance. Before relying on it, do one real end-to-end pass: Setup with real
  credentials, refresh My Tasks, start/stop a timer, edit an entry, and Sync — the
  create-vs-update worklog logic and Jira's accepted date/ADF comment format need a
  real round trip.
