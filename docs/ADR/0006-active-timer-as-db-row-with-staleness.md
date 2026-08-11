# 0006: Persist the active timer as a DB row; treat staleness, not presence, as the crash-recovery signal

**Status:** Accepted — 2026-08-10

## Context

The spec's non-functional requirements ask that the app "handle window/focus changes
without losing timer data" and either auto-save the timer state or prompt the user to
resolve it on next launch if the app is closed (or crashes) with an active timer. This
was left as an explicit open technical question (#4: "storage format for 'active
timer' state in case of a forced application shutdown").

## Decision

There is no separate "active timer" file or table. A running timer *is* a
`time_entries` row with `ended_at IS NULL` — the same table that stores every other
time record. `timer::engine::get_running` queries for it directly; a partial unique
index (`idx_time_entries_single_running`, `WHERE ended_at IS NULL`) backstops the
"only one running timer" invariant at the DB level, in addition to the app-level
transaction in `timer::engine::start` that stops-and-saves any prior running entry
before inserting the new one.

On launch, the frontend calls `get_active_timer` once. Finding a running entry is
treated as the **normal** case (a resumed session, or an intentionally long-running
timer) and shown as running with no interruption — not a blocking "resolve this"
prompt on every reopen. The returned DTO includes a server-computed `isStale` flag
(`timer::engine::is_stale`, threshold `STALE_THRESHOLD_SECS = 12h`); only past that
threshold does the UI show a non-blocking banner ("still running — keep going or stop
now?"). This is what actually answers the spec's crash-recovery requirement, without
punishing the common case of the app being closed and reopened while a timer is
legitimately still running (e.g. a long meeting, or overnight).

## Consequences

- Crash recovery costs nothing extra to implement or keep in sync — the running
  timer's state is exactly as durable as every other time entry, because it's the
  same table and the same SQLite transaction guarantees.
- The 12-hour staleness threshold is a judgment call, not something the spec
  specifies numerically. It can be changed by editing one constant
  (`timer::engine::STALE_THRESHOLD_SECS`) without any data migration.
- If a truly silent data-loss scenario were needed (e.g. the DB file itself is
  corrupted mid-write), this design does not add extra protection beyond SQLite's own
  durability guarantees — that risk is accepted as out of scope.

## Alternatives considered

- **A dedicated `active_timer.json` (or similar) sidecar file** — rejected: adds a
  second source of truth that can drift from the `time_entries` table, and buys
  nothing the partial unique index + transaction don't already provide.
- **Always show a blocking "resolve this" dialog on launch if a timer is running** —
  rejected: would fire on every normal app reopen while a timer is running, not just
  on an actual crash/left-overnight case, which is a worse experience for the common
  path.
