# 0031: Allow editing a completed workday session's start/end time

**Status:** Accepted — 2026-09-17

## Context

ADR-0017 gave the workday clock the same live start/stop symmetry as the timer and
break widgets, with no manual-duration entry path. The same failure mode ADR-0020
identified for breaks applies at the day level too, and is arguably more disruptive
there: forgetting to click "End Workday" for a few hours inflates `worked_seconds` for
the rest of the day, and until now there was no way to correct it — `work_days` had no
update path (`db/work_days_repo.rs` only ever supported insert-once/stop-once), and
`WorkdayWidget` never rendered anything for an already-ended session at all.

ADR-0020 explicitly flagged this as a known gap: "Breaks belonging to an already-ended,
no-longer-active `work_days` session are only reachable through this edit path if some
future view lists past workdays; `WorkdayWidget` today only renders
`activeWorkday.breaks`." Fixing a forgotten-to-stop workday hits exactly this gap —
once `end_workday` is called (however late), `get_active_workday` returns `None` and
the session disappears from the widget entirely, with no way to reopen it.

## Decision

Add a narrowly-scoped edit path for a single `work_days` row, following the same
layering and edit-gating precedent as ADR-0020's break edit: **a currently-running
workday cannot be edited** — its lifecycle stays owned exclusively by
`start_workday`/`end_workday` — so correcting a forgotten clock-out is the same
two-step flow as a forgotten break: end it (however late), then fix its start/end time.

- `db::work_days_repo::update_workday(conn, id, work_date, started_at, ended_at)` — a
  plain three-column `UPDATE`, no different in shape from `update_break`.
- `workday::engine::update_workday(conn, id, started_at, ended_at)` holds the business
  rules: the workday must exist, must not currently be running, its end must be
  strictly after its start, and — the reverse of `update_break`'s own containment
  check — the new bounds must still fully contain every break recorded against it (a
  break outliving its parent workday would silently corrupt `worked_seconds`, same
  reasoning as ADR-0020, just inverted). `work_date` is recomputed from the corrected
  `started_at` via the existing `local_date` helper, so an edit that shifts a session
  across a local-midnight boundary still files under the right calendar day — `work_date`
  is otherwise never touched after `start_workday` sets it.
- `commands::workday::update_workday` — thin Tauri wrapper parsing the two RFC3339
  timestamp strings, following the same `parse_dt` pattern as `update_break`.
- `commands::workday::get_todays_workday_sessions` — the piece ADR-0020 left as a gap:
  lists every `work_days` row (plus its own breaks) for today's local date, not just
  the currently active one, so an already-ended session becomes reachable again. This
  closes the gap for split-shift breaks too, not just workday bounds.
- Frontend: `WorkdayWidget` now renders a "Today's sessions" line listing every
  *completed* session as a clickable link-button (`start–end (duration)`), opening
  `EditWorkdayForm` (mirroring `EditBreakForm`'s date/start-time/end-time fields, same
  `combine`/`toDateInput`/`toTimeInput` helpers). The currently-running session isn't
  in this list — it's still shown by the existing live display.

Unlike `time_entries`, there's no sync-status gate to consider — `work_days` carries no
`is_synced`/`jira_worklog_id` columns, so "not currently running" plus "still contains
its breaks" are the only preconditions.

## Consequences

- Fixing a forgotten clock-out now takes two actions (End Workday, then Edit) instead
  of one, but this avoids a second, editing-triggered way to close a workday —
  `end_workday` remains the only path that transitions a session from open to closed,
  consistent with ADR-0020's identical tradeoff for breaks.
- `get_todays_workday_sessions` is a new always-available read path into every session
  recorded today, which also makes split-shift breaks under an already-ended session
  editable again — a strict improvement over today's dead end, and exactly the gap
  ADR-0020 called out.
- No history view exists for days before today — this covers "noticed it the same day,
  before or after ending the session" (the common case, and what "forgot to stop the
  workday timer" almost always means in practice) but not correcting a session from a
  prior day. That's an acceptable gap for this pass, same scope boundary ADR-0020 drew
  for breaks.
- Editing a session's bounds to exclude one of its own recorded breaks is rejected with
  a validation error rather than silently producing a wrong `worked_seconds` — mirrors
  ADR-0020's `BreakOutsideWorkday` check, inverted.

## Alternatives considered

- **Let `end_workday` take an optional custom end timestamp** (end "as of" a chosen
  past time instead of always `now`) — rejected for the same reason ADR-0020 rejected
  free-form break edits: it conflates "stop" with "correct", and still wouldn't help
  when the mistake is only noticed after the (wrong) end has already been recorded.
  The two-step stop-then-edit flow handles both cases uniformly.
- **A full workday history view** (browse and edit any past day, not just today) —
  deferred as unnecessary scope for the case actually reported (a same-day forgotten
  clock-out); `get_todays_workday_sessions` is a strict subset of what such a view
  would need, so it isn't foreclosed later.
