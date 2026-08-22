# 0025: UI preferences live in a key/value table, behind a Settings modal

**Status:** Accepted — 2026-08-21

## Context

The app had no user-adjustable settings at all — only the Jira connection, which lives
in the `settings` table (one row, fixed columns) plus the keychain. The first real
preferences arrived together: how many rows each task panel shows before scrolling
(the panels were hard-coded to 5 and 4 rows in CSS), and whether My Tasks starts
filtered to the current sprint (fixed to on by ADR-0024).

Neither belongs on the `settings` row: that row is the Jira connection, and every
future preference added there costs a migration plus a column that is meaningless to
the rest of the code.

## Decision

- A `preferences` table of `(key TEXT PRIMARY KEY, value TEXT NOT NULL)`, migration
  `004_preferences.sql`, read and written through `db::preferences_repo`. Values are
  text; the reader parses and **owns the default** — `get_i64` falls back when the key
  is missing *or* unparseable, so a hand-edited or stale value degrades to the default
  rather than failing the read. A preference is a convenience, never a reason for a
  command to error.
- `commands::preferences` exposes one typed DTO over that store
  (`get_preferences` / `save_preferences`) rather than a generic key/value API, so the
  frontend still sees a checked shape and the valid range for each setting is enforced
  in one place. Row counts are capped at 25: a panel taller than that pushes the timer
  and History off-screen, which is worse than scrolling inside the panel.
- **Settings** is a header button, left of Sync, sharing the `link-button` styling with
  Reconfigure — the two are the same class of action (app-level configuration) and read
  as a pair beside the primary Sync button.
- The panels take their height from the preference inline (`rows * 37px`), keeping the
  `.task-list-capped` scroll behaviour and dropping the fixed `.my-tasks-list` /
  `.favorites-list` heights.
- `MyTasksPanel` is keyed on the sprint-default preference so that the toggle picks it
  up when it arrives: preferences load asynchronously after first paint, and the
  toggle's `useState` seed would otherwise be stuck at the value it first rendered with.
  Unticking remains a per-session look at the full list; the preference decides each
  launch.

## Consequences

- Preferences survive reinstalls and webview data clearing, since they sit in the same
  SQLite file as the entries. They are also per-machine, like the rest of the DB.
- Adding the next preference is a key, a DTO field and a form control — no migration.
  The cost is that the table has no schema: a typo in a key name reads as "unset", and
  nothing stops two callers disagreeing about a value's format. The typed command layer
  is what keeps that contained, so preferences should keep going through it rather than
  reading the repo directly.
- Row counts are per-panel rather than one shared number, because My Tasks and
  Favorites are used differently — a long sprint list next to a short pinned list is
  the normal case.

## Alternatives considered

- **`localStorage`** — no backend work at all, but it is webview state: cleared with
  site data, invisible to the Rust side, and inconsistent with every other piece of
  state in the app living in SQLite.
- **Columns on the `settings` table** — a schema for each preference and a migration
  every time one is added, for values that are all small scalars.
- **A settings *view* instead of a modal** — the app has one screen plus Statistics;
  a third tab for two fields would be heavier than the modal pattern already used for
  editing entries and breaks.
