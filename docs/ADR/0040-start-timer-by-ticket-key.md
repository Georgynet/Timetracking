# 0040: Start the timer by ticket key, from the window and the tray

**Status:** Accepted — 2026-10-29

## Context

The timer could only be started for a ticket in My Tasks or Favorites (the start form's
dropdown, the tray's "Start timer" submenu). Reviewing someone else's ticket doesn't fit
either list, so review time was entered afterwards as a manual entry — from memory, and
sometimes not at all.

## Decision

The timer can be started — or, while one runs, switched — by typing a ticket key.

- The start form gets a "By key…" link that swaps the ticket dropdown for a key field;
  while a timer runs, "Switch to key…" opens the same fields in a row under it. Enter
  starts, Escape cancels.
- The field accepts a bare key (`proj-123`) or a pasted Jira link; the frontend pulls
  the key out (`lib/jira.ts::extractIssueKey`, last key-shaped match wins).
- The key goes through the existing `resolve_task_by_key` (ADR-0023): the ticket is
  fetched from Jira, so a typo fails immediately rather than at sync time, and is
  stored as a local task. Then the normal `start_timer` runs, so the single-running-timer
  rule, stop paths (window, tray, switching) and sync are all unchanged. No new command.
- The comment field is prefilled with "CR" when the key form opens with an
  empty comment, since reviews are what this is for. It's editable and is removed again
  on Cancel if untouched.
- The tray's Start/Switch submenu gets a "Ticket key…" item at the top. A native menu
  can't take text input, so the item shows the window, switches to the tracker tab and
  emits `tray-start-by-key`; the frontend opens and focuses the key field. MainView
  passes this to `TimerWidget` as a request flag that the widget clears, not a counter,
  because the widget remounts when returning from Statistics and must not reopen the
  field each time.

## Consequences

- Reviews can be tracked live instead of reconstructed later.
- A ticket started by key is an ordinary local task, so after the first time it appears
  under Recent and Resume in the tray (ADR-0036, ADR-0027) and can be restarted in one
  click.
- Starting by key needs Jira to be reachable; offline, it fails with Jira's error and
  the timer isn't started.
- The "CR" default is a hardcoded string, not a preference.

## Alternatives considered

- **Read the key from the clipboard in the tray menu** — rejected for now: one click when
  it works, but the item would appear and disappear with the clipboard contents and the
  menu would have to be rebuilt on every open to stay accurate.
- **A separate "review" entry type or flag** — rejected: a review is ordinary time on a
  ticket, and the worklog comment already says what it was.
- **Start without asking Jira** — rejected: an unknown key would only surface as a sync
  failure, and the entry would have no summary in History.
