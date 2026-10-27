# 0038: macOS notifications — action results and configurable reminders

**Status:** Accepted — 2026-10-26

## Context

With the window hidden in the menu bar (ADR-0037), the app has no way to tell the user
anything: a sync started from the tray reports nowhere, and the classic time-tracking
mistakes — a timer left running, clocked in without a timer, a break never ended, a
workday never closed — go unnoticed until the numbers are wrong. The only existing
signal is the in-window "stale timer" banner after 12 hours (ADR-0006).

## Decision

macOS only (like ADR-0037), via `tauri-plugin-notification`, in
`src-tauri/src/notifications/`:

**Action results**
- Sync started from the tray: "N synced" or "N synced, M failed".
- Ending the workday (tray or UI) with unsynced entries: "N entries not synced".

**Reminders**, checked every 30 seconds by a background thread
(`reminders::due`, pure and unit tested):
- *Timer running for X* (default 4h) — repeats each time another X has passed.
- *Clocked in with no timer for X* (default 30m) — idle measured from the latest of
  clock-in, end of the last break, and end of the last time entry; silent on a break;
  once per idle stretch.
- *Break lasting X* (default 1h) — once per break.
- *Workday still running after working X* (default 9h) — once per day. Measured as
  **worked time today** (all sessions, breaks excluded), not a clock time: start times
  vary from day to day, so "after 19:00" would be wrong as often as right.

Every type can be switched off and every threshold changed in Settings → Notifications
(durations in the same Jira-style format as the manual entry form, `30m`, `1h 30m`).
Stored as `notify.*` keys in the preferences table (ADR-0026), via their own
`get_/save_notification_settings` commands rather than more arguments to
`save_preferences`. All types are on by default. The section is hidden where
`notifications_supported` is false.

What was already shown is kept in memory (`ReminderMemory`); after a restart a
still-true reminder may fire once more.

## Consequences

- Mistakes surface while they're still cheap to fix, without the window open.
- Notifications only inform: desktop notifications in the plugin can't carry action
  buttons, so acting on one goes through the tray or the window.
- In `tauri dev` (an unbundled binary) macOS may not show notifications, or attributes
  them to another app; they need to be checked in a bundled build.

## Alternatives considered

- **End-of-day reminder at a fixed clock time** — rejected (see above).
- **Persisting `ReminderMemory`** — rejected: one possible repeat after a restart isn't
  worth a table.
- **Linux too** — deferred: notification daemons differ and can't be verified here.
- **Off by default** — rejected: reminders nobody discovers don't help.
