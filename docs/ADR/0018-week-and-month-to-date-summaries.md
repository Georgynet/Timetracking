# 0018: Week-to-date and month-to-date summaries, generalized from the daily one

**Status:** Accepted — 2026-08-12

## Context

ADR-0017 added `workday::engine::daily_summary`, comparing worked time against
Jira-logged time for a single local calendar date. The user also wanted to see the same
worked-vs-logged comparison for "this week" and "this month" next to the existing
"Today: worked … · logged … · diff …" line, without duplicating the summing logic for
each new time granularity.

## Decision

`daily_summary`'s body was generalized into `workday::engine::range_summary(conn, from,
to, now)`, which sums `worked_seconds` across every `work_days` row whose `work_date`
falls in the inclusive local-date range `[from, to]`, and `logged_seconds` from
`time_entries` within that range's local-day UTC bounds — exactly the same rule as
before, just iterated over a range instead of a single date. `daily_summary` now just
calls `range_summary(conn, date, date, now)`, so the existing tests and behavior for
"today" are unchanged.

Two new commands, `get_week_summary` and `get_month_summary`, call `range_summary` with
`from` computed by two new pure helpers: `workday::engine::week_start` (the Monday of
the local week containing today) and `month_start` (the 1st of the local month
containing today), and `to` always today — i.e. both are strictly week-to-date /
month-to-date, not full-week/full-month totals that would include future days.

On the frontend, `WorkdayWidget` derives a live-ticking week/month worked total instead
of showing the raw (fetch-time-frozen) `RangeSummary.workedSeconds`: since both the week
and month ranges include today, and `dailySummary`/`weekSummary`/`monthSummary` are
always fetched together (`loadPeriodSummaries`), the stale "today" slice inside each
range total is swapped for the same live `workedToday` value the "Today" line already
computes per second. This keeps the whole summary line visibly consistent while a
workday is running, without polling the backend every second for the week/month totals
too. The week/month figures are appended to the existing "Today: worked … · logged … ·
diff …" line as `· This week: worked … · This month: worked …` — worked-only, no
logged/diff, since the line is meant as a quick at-a-glance total rather than a repeat
of the full daily comparison.

## Consequences

- No schema or migration change — this is a query-shape generalization over the
  ADR-0017 tables, not a new data model.
- Week-to-date and month-to-date share the exact same worked/logged/diff semantics as
  the daily summary; there is no separate "definition" of worked time to keep in sync.
- The frontend's live-carry-forward math assumes the three summaries are fetched
  together; if a caller ever fetches `weekSummary` without `dailySummary` in the same
  round-trip, the derived live total would double-count or under-count today's slice.
  This is enforced by `loadPeriodSummaries` being the only place any of the three are
  loaded from, not by a type-level guarantee.

## Alternatives considered

- **A single `get_summary(from, to)` command**, with the frontend computing week/month
  start dates itself — rejected: `CLAUDE.md` places business logic in Rust, and "what
  counts as the start of the current week" is exactly that kind of logic, not a view
  concern.
- **Polling week/month summaries every second** like the "Today" elapsed clock —
  rejected as unnecessary backend load for a number that only needs to be
  visually consistent with "Today", not independently live; the frontend-side
  derivation achieves the same visible effect for free.
