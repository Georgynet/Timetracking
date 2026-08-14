# 0020: Allow editing a completed break's start/end time

**Status:** Accepted — 2026-08-14

## Context

ADR-0017 introduced live Start/End Break buttons as the only way to record a break,
explicitly rejecting manual break-duration entry so the break UI would mirror the
timer widget's start/stop symmetry. In practice, it's easy to forget to click "End
Break" promptly — by the time the user remembers, the recorded break has run far
longer than the real one, which throws off `worked_seconds` and the worked-vs-logged
diff for the rest of the day. Until now there was no way to correct this short of
living with the wrong number: `work_breaks` had no update path at all (`db/work_days_repo.rs`
only ever supported insert-once/stop-once), and `WorkdayWidget` rendered each break as
plain, non-interactive text.

## Decision

Add a narrowly-scoped edit path for a single `work_breaks` row, mirroring the
edit-gating precedent already established for `time_entries` (ADR-0013): **a
currently-running break cannot be edited** — its lifecycle stays owned exclusively by
`start_break`/`end_break` — so correcting a forgotten-to-stop break is a two-step
flow: stop it (however late), then fix its start/end time. This keeps "is a break
open" a single, unambiguous invariant rather than letting an edit implicitly
close one.

New pieces, following the existing repo → engine → command → frontend layering:

- `db::work_days_repo::update_break(conn, id, started_at, ended_at)` — a plain
  two-column `UPDATE`, no different in shape from `stop_break`.
- `workday::engine::update_break(conn, id, started_at, ended_at, now)` holds the actual
  business rules: the break must exist, must not currently be running, its end must
  be strictly after its start, and — new relative to any `time_entries` edit rule —
  both bounds must fall within the parent `work_day`'s own span (`now` standing in for
  a still-open workday's `ended_at`, same convention as `worked_seconds`/`break_seconds`
  elsewhere in this module). A break outliving the workday it belongs to would silently
  corrupt `worked_seconds`, so this is checked at the engine layer rather than left to
  the caller.
- `commands::workday::update_break` — thin Tauri wrapper parsing the two RFC3339
  timestamp strings, following the same `parse_dt` pattern already used in
  `commands::entries`.
- Frontend: each *completed* break rendered by `WorkdayWidget` is now a clickable
  link-button opening `EditBreakForm` (a small modal reusing the
  `toDateInput`/`toTimeInput`/`combine` helpers, now hoisted from `ManualEntryForm`
  into `lib/format.ts` so both forms share them) with date/start-time/end-time fields.
  A still-running break renders as plain text, same as before.

Unlike `time_entries`, there's no sync-status gate to consider — `work_breaks` carries
no `is_synced`/`jira_worklog_id` columns by construction (ADR-0017), so "not currently
running" is the only precondition.

## Consequences

- Fixing a forgotten break now takes two actions (End Break, then Edit) instead of
  one, but this avoids adding a second, editing-triggered way to close a break —
  `end_break` remains the only path that transitions a break from open to closed.
- The parent-workday bounds check means a break can't be edited to extend past its
  workday's own end (or before its start) — attempting to do so surfaces as a
  validation error asking the user to fix the numbers, rather than silently producing
  a negative `worked_seconds` that `worked_seconds`'s existing `.max(0)` clamp would
  otherwise mask.
- Breaks belonging to an already-ended, no-longer-active `work_days` session are only
  reachable through this edit path if some future view lists past workdays;
  `WorkdayWidget` today only renders `activeWorkday.breaks`, so this change covers the
  common case (noticing and fixing the mistake the same day, before or after clocking
  out) but not editing a break from a previous day. That's an acceptable gap for this
  pass — no such history view exists for workdays/breaks at all yet, unlike
  `time_entries`' `HistoryList`.

## Alternatives considered

- **Auto-close a running break when a new action starts** (e.g. treat "End Workday"
  or app restart as an implicit edit-and-close with a guessed duration) — rejected:
  guessing a duration is worse than asking the user to supply the correct one, and
  ADR-0017 already established that clocking out is the only thing allowed to
  implicitly close an open break.
- **Free-form duration edit** (change a break's length without pinning both
  timestamps) — rejected in favor of editing both `started_at`/`ended_at` directly,
  consistent with how `time_entries` editing works (ADR-0014) and simpler to reason
  about than a duration that has to be re-anchored to one end or the other.
