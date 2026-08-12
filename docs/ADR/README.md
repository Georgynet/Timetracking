# Architecture Decision Records

Records of the significant technical decisions made while building the time tracker
(see `../../spec_time_tracker_jira.md` for the product spec these implement against).

Format: lightweight [Nygard-style](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)
ADRs — Context, Decision, Consequences, and Alternatives considered where relevant.
Once accepted, an ADR is not edited to reflect new decisions — a later ADR supersedes
it instead, so the history of *why* stays intact.

| # | Title | Status |
|---|---|---|
| [0001](0001-tauri-v2-as-application-shell.md) | Use Tauri v2 as the application shell | Accepted |
| [0002](0002-react-typescript-vite-frontend.md) | React + TypeScript + Vite for the frontend | Accepted |
| [0003](0003-rusqlite-over-sqlx.md) | Use `rusqlite` instead of `sqlx` for local storage | Accepted |
| [0004](0004-os-keychain-via-keyring-crate.md) | Store the Jira API token in the OS keychain via the `keyring` crate | Accepted |
| [0005](0005-jira-client-trait-and-mocked-testing.md) | Abstract Jira access behind a trait; verify via mocks, defer live Jira testing | Accepted |
| [0006](0006-active-timer-as-db-row-with-staleness.md) | Persist the active timer as a DB row; treat staleness, not presence, as the crash-recovery signal | Accepted |
| [0007](0007-is-synced-means-currently-in-sync.md) | Redefine `is_synced` as "currently in sync" to support worklog updates without a schema change | Accepted |
| [0008](0008-sync-engine-per-record-isolation.md) | Sync engine: per-record failure isolation, chronological order, no retry queue | Accepted |
| [0009](0009-hard-delete-only-for-never-synced-manual-entries.md) | Allow hard-delete only for manual entries that were never synced | Accepted |
| [0010](0010-programmatic-tray-with-linux-fallback.md) | Build the system tray programmatically with a window-only fallback for Linux | Accepted |
| [0011](0011-log-safety-by-construction.md) | Guarantee log safety by construction, not a redaction layer | Accepted |
| [0012](0012-separate-command-impl-from-tauri-command-wrappers.md) | Separate pure command-impl functions from `#[tauri::command]` wrappers for testability | Accepted |
| [0013](0013-running-timer-not-editable-outside-start-stop.md) | The running timer cannot be edited or deleted outside start/stop | Accepted |
| [0014](0014-sync-status-gates-edit-and-delete.md) | Sync status gates edit/delete: unsynced is fully mutable, synced is permanent | Accepted |
| [0015](0015-task-summary-denormalized-onto-time-entry.md) | Denormalize the ticket summary onto `TimeEntry` for the history tooltip | Accepted |
| [0016](0016-jira-links-via-opener-plugin.md) | Link tickets to Jira via the `opener` plugin, not plain anchor navigation | Accepted |
| [0017](0017-local-only-workday-tracking-with-breaks.md) | Local-only workday tracking (start/end + breaks), compared against Jira-logged time | Accepted |
| [0018](0018-week-and-month-to-date-summaries.md) | Week-to-date and month-to-date summaries, generalized from the daily one | Accepted |
