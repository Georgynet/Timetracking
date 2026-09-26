# 0032: Date navigation for past workday sessions

**Status:** Accepted — 2026-09-26

## Context

ADR-0031 made a completed `work_days` session editable, but only for today:
`get_todays_workday_sessions` hard-coded the local date to `Utc::now()`, and
`WorkdayWidget` rendered a single "Today's sessions" line with no way to look further
back. ADR-0031 explicitly flagged this as an accepted gap for that pass: "No history
view exists for days before today... an acceptable gap for this pass."

In practice this gap bites on exactly the case the whole edit path exists for: forgetting
to click "End Workday". If that's noticed the next day rather than the same day — a
common way to notice it, since the app isn't necessarily open when it happens — the
previous day's session had already scrolled out of reach with no way to correct it.

## Decision

Generalize the today-only sessions list to any local calendar date, and let
`WorkdayWidget` navigate to it, instead of building the fuller history view ADR-0031
deferred (browsing/editing arbitrary past days, alongside `time_entries`' `HistoryList`)
which is more scope than this gap needs.

- `commands::workday::get_todays_workday_sessions` is replaced by
  `get_workday_sessions(date: Option<String>)`, taking the same optional-date-defaulting
  pattern already used by `get_daily_summary` — `None` defaults to today in the local
  timezone via `engine::local_date`. No change to `workday::engine` or the repo layer:
  `work_days_repo::work_days_for_date` already took a date and needed no changes.
- `update_workday`'s own rules (ADR-0031: not currently running, end after start, must
  still contain all recorded breaks) are date-agnostic already, so editing a past day's
  session works unchanged once it's reachable.
- Frontend: `WorkdayWidget` gets a Prev/Next day nav row above the sessions list
  (`workdaySessionsDate` state in the Zustand store, `shiftDate`/`localDateString`
  helpers in `lib/format.ts` for local-calendar day arithmetic that doesn't go through
  UTC ISO strings). "Next" is disabled once the viewed date reaches today — there's
  nothing to navigate forward into. An empty day now renders "No completed sessions"
  instead of the section disappearing, since the whole point is to be able to navigate
  *to* an empty-looking day and find the session that's actually there.
- Starting/ending a workday always happens today regardless of which date is currently
  being viewed, so `startWorkday`/`endWorkday` in the store explicitly reload today's
  sessions rather than whatever `workdaySessionsDate` happens to be — the same reasoning
  ADR-0031 already applied to `loadActiveWorkday`/`loadPeriodSummaries` there.

The still-running workday continues to be reachable only through the live
`activeWorkday` display, unaffected by which date the sessions list is navigated to —
same "end it first, then correct it" two-step flow ADR-0031 established, since a
still-running session's own `work_date` may not even be today's if it's been left open
overnight.

## Consequences

- Closes ADR-0031's "noticed the next day" gap: a workday left running overnight can now
  be ended and then found under its actual start date, not just under today.
- Still bounded: this is day-by-day navigation from today backwards, not a searchable or
  paginated history view. Jumping to an arbitrary distant date takes that many clicks —
  acceptable for the "noticed a day or two late" case this targets, but a real history
  view (a `HistoryList`-style browser for `work_days`) remains future scope if that turns
  out to be needed.
- One additional round-trip per day navigated, same cost shape as `get_daily_summary`.

## Alternatives considered

- **A full workday history view**, as ADR-0031 already sketched as future scope — still
  deferred; day-by-day Prev/Next is enough for "forgot yesterday" without building a
  second history browser alongside `HistoryList`.
- **A date picker instead of Prev/Next buttons** — rejected for now as more UI than the
  common case (one or two days back) needs; Prev/Next is one click for the case that
  actually shows up, and nothing forecloses adding a picker later if arbitrary jumps turn
  out to be needed.
