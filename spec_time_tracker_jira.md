# Technical Specification
## Cross-platform Time Tracking Application with Jira Sync

**Document version:** 1.0
**Date:** 2026-08-10
**Status:** Draft for MVP implementation

---

## 1. Project Goal

Build a desktop application (macOS, with Linux support in mind) for developer time tracking, with manual synchronization of logged time to Jira Cloud. The application must allow:

- tracking time on tasks assigned to the current user in Jira;
- running a timer against "favorite" tickets for general-purpose logging (meetings, daily standups, feedback sessions, etc.);
- manually editing tracked time;
- storing the full time-log history locally, regardless of sync status;
- pushing worklogs to Jira on demand, via a "Sync" button.

---

## 2. Technology Stack

| Component | Choice |
|---|---|
| Application framework | **Tauri** (Rust backend + web frontend) |
| Frontend | React/Vue/Svelte (team's choice) |
| Backend logic | Rust (timer, database access, Jira API HTTP client) |
| Local storage | SQLite (embedded, via `rusqlite` or `sqlx`) |
| Target platforms | macOS (primary), Linux (secondary, plan for tray/DE differences from the start) |
| Jira API | Jira Cloud REST API v3 |
| Authentication | Basic Auth (user email + API token) |

**Note on Linux:** system tray behavior differs across desktop environments (GNOME/KDE/etc.). A fallback UI mode (regular window instead of tray-only) must be planned for cases where the tray is unavailable or unreliable.

---

## 3. Data Model (local database)

### 3.1. `settings` table (user configuration)
- `jira_base_url` — instance URL (e.g. `https://company.atlassian.net`)
- `jira_email` — user's email
- `jira_api_token` — API token (must be stored in the OS secret storage/keychain, not in plain text in the database)

### 3.2. `tasks` table (cached Jira tasks)
- `id` (local)
- `jira_key` (e.g. `PROJ-123`)
- `summary` — task title
- `is_favorite` — boolean, marked as favorite
- `is_assigned_to_me` — boolean, fetched as "my" task
- `last_synced_at`

### 3.3. `time_entries` table (time records)
- `id`
- `task_id` (FK → tasks)
- `started_at`
- `ended_at`
- `duration_seconds`
- `comment` (optional, nullable)
- `is_synced` — boolean, whether sent to Jira
- `jira_worklog_id` — nullable, Jira worklog ID after sync
- `created_manually` — boolean (manually created vs via timer)
- `edited_at` — nullable, timestamp of last manual edit

Important: records are **never deleted** and are **not automatically modified** after sync — they remain in the local database indefinitely for future statistics, regardless of `is_synced` status.

---

## 4. Functional Requirements

### 4.1. Jira Authentication
- On first launch — a form to enter: instance base URL, email, API token.
- Credentials are validated via a test request to the Jira API (e.g. `GET /myself`).
- The token is stored in the OS-protected secret storage (macOS Keychain / Linux Secret Service), not in the database file.

### 4.2. Fetching Tasks from Jira
- Triggered by a "Refresh task list" button — request via JQL: `assignee = currentUser() AND resolution = Unresolved` (or similar, to be finalized during implementation).
- The task list is cached locally in the `tasks` table.
- Display: task key, title, (optionally) status/project.

### 4.3. Favorite Tasks
- The user can add **any** existing Jira ticket to "Favorites" (including unassigned ones), via:
  - search by ticket key (e.g. `PROJ-999`), or
  - search by JQL/free-text query.
- Favorite tasks are shown in a dedicated UI section, independent of the "my tasks" list.
- Typical use case: shared "container" tickets such as `TEAM-1` (Daily meetings), `TEAM-2` (Feedback sessions), etc. — these tickets are unassigned but available to the whole team for logging time.

### 4.4. Timer
- Only **one** timer can be active at any given time.
- Starting a timer on a new task automatically stops and saves the currently active timer as a completed record (`time_entries`).
- Active timer status is indicated in the UI (and in the menu bar/tray, if technically feasible).

### 4.5. Manual Time Editing
- Ability to manually create a time record (select ticket, date, start/end time or simply a duration).
- Ability to edit existing records (including ones created via the timer), including:
  - changing the ticket a record is linked to (in case the user forgot to switch the timer);
  - changing start time/end time/duration.
- If a record has already been synced (`is_synced = true`) and is subsequently edited, re-synchronization must be handled correctly (updating the existing worklog in Jira rather than creating a duplicate). This needs to be verified against the Jira API during implementation (`PUT /issue/{issueIdOrKey}/worklog/{id}`).

### 4.6. Jira Sync
- Sync is **manual only**, triggered by a "Sync" button.
- On click — all unsynced records (`is_synced = false`) are sent to Jira as worklogs via `POST /issue/{issueIdOrKey}/worklog`.
- The worklog comment is optional and not required.
- On successful submission — the record is marked `is_synced = true`, and `jira_worklog_id` is stored.
- Time rounding before submission is **not implemented in the MVP**; it is reserved as a possible future setting (e.g. rounding to 15-minute increments).
- Sync error handling: if an individual record fails to sync (network error, invalid ticket, etc.), it remains `is_synced = false`, a clear error is shown to the user, and the remaining records continue to sync.

### 4.7. Storage and Offline Mode
- The application is fully functional offline for time tracking (creating/editing records).
- Sync requires an internet connection; if unavailable, a clear error message is shown, and sync can be retried later without any data loss.
- All records (synced and unsynced) are stored locally indefinitely — the foundation for future statistics/reporting (reporting itself is out of scope for the MVP).

---

## 5. Non-Functional Requirements

- Installation is per-developer, individual; usage is opt-in (it does not forcibly replace any existing team time-tracking process).
- API tokens and other sensitive data must never appear in application logs.
- The application must handle window/focus changes without losing timer data (e.g. if the app is closed with an active timer, either the state must be auto-saved, or the user must be prompted to resolve it on next launch).

---

## 6. Explicitly Out of Scope for MVP (possible future work)

- OAuth 2.0 authentication instead of API token.
- Automatic/periodic synchronization.
- Time rounding before sending to Jira.
- Notifications/reminders (long work sessions, pending sync, etc.).
- Report export (CSV/PDF).
- Support for multiple Jira instances simultaneously.
- Support for tasks where the user is Reporter/Watcher (currently Assignee only).
- Multi-user configuration within a single installation.

---

## 7. Open Technical Questions for Implementation

These do not block the start of development but must be resolved along the way:

1. Exact mechanism for updating a worklog when an already-synced record is edited (update vs. re-create).
2. Behavior on partial sync failure (retry logic, order of record submission).
3. Exact tray implementation on Linux — determine minimally supported environments (GNOME, KDE) or design a universal fallback UI.
4. Storage format for "active timer" state in case of a forced application shutdown (crash recovery).
