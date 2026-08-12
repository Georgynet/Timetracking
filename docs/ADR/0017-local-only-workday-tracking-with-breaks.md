# 0017: Local-only workday tracking (start/end + breaks), compared against Jira-logged time

**Status:** Accepted — 2026-08-11

## Context

`time_entries` only records time logged *against a Jira ticket*. There was no way to
answer "how much of my actual working day made it into a ticket at all?" — the user
wanted a simple clock-in/clock-out flow with breaks, stored strictly in the local
database (never synced to Jira), so the app can show the gap between total hours
worked and hours logged to Jira.

The codebase already solved "exactly one active X at a time, crash-safe, no sidecar
state" for the timer (ADR-0006): a running timer *is* a `time_entries` row with
`ended_at IS NULL`, backstopped by a partial unique index. This feature needed the same
property for two new concepts — an open workday and an open break within it — so it
reuses that pattern rather than inventing a new one.

## Decision

Two new tables, `work_days` and `work_breaks` (migration
`002_work_days.sql`), each following the ADR-0006 shape: an open row
(`ended_at IS NULL`) *is* the active state, with a partial unique index
(`idx_work_days_single_running`, `idx_work_breaks_single_running`) enforcing at most
one open row per table. Neither table has any Jira-related column (no `is_synced`, no
`jira_worklog_id`) — this data structurally cannot be pushed to Jira, matching the
"strictly local" requirement by construction rather than by a rule someone has to
remember not to break.

`work_days.work_date` stores the *local* calendar date (`workday::engine::local_date`,
derived from `chrono::Local`), not UTC — this is a single-user desktop app, and
"workday" is inherently a local-calendar-day concept. Multiple `work_days` rows can
share the same `work_date` (a split shift, e.g. lunch spanning a full clock-out); the
daily summary sums across all of them rather than assuming one row per day.

Unlike the timer, starting a workday/break while one is already open is **rejected**
(`WorkdayError::AlreadyRunning` / `BreakAlreadyRunning`), not auto-switched — there is
no "switch tickets" equivalent for clocking in twice, so silently ending the first
session would more likely hide a mistake than fix one. Ending a workday **does**
auto-close an open break at the same timestamp (`workday::engine::end_workday`),
since clocking out unambiguously means you're no longer on that break either.

`workday::engine::daily_summary(conn, date, now)` computes, for a given local date:
`worked_seconds` (sum of `workday::engine::worked_seconds` — span minus breaks, with
`now` standing in for any still-open `ended_at` so an in-progress day/break counts
live) across every `work_days` row on that date, and `logged_seconds` (sum of
`time_entries.duration_seconds` for entries whose `started_at` falls within that local
day's UTC bounds, via the existing `time_entries_repo::list_entries`). `diff_seconds`
is `worked − logged`.

## Consequences

- Day boundaries depend on the local timezone of the machine running the app —
  acceptable for a single-user, single-machine desktop tool, and consistent with how
  the timer's own "today" concept would work if it needed one.
- Split shifts are supported by summing multiple `work_days` rows per date rather than
  requiring the UI to merge them into one row; the daily summary is correct either way.
- Because the schema carries no sync-related columns at all, there is no code path
  through which this data could accidentally be sent to Jira — the local-only
  guarantee doesn't rely on remembering to check `is_synced` anywhere.
- A DST fold (an ambiguous local time occurring twice) resolves to the earlier
  instant in `local_day_bounds_utc`; this is a twice-a-year best-effort choice, not a
  correctness guarantee, and was judged not worth more machinery for an MVP feature.

## Alternatives considered

- **Manual break-duration entry** (e.g. "log 30 min for lunch" after the fact) —
  rejected in favor of live Start/End Break buttons, so the break UI mirrors the
  existing timer widget's start/stop symmetry instead of introducing a different
  interaction model for a closely related concept.
- **Derive "worked time" from the sum of ticket timers** instead of a separate
  clock — rejected: this is exactly the thing being measured against. If worked time
  were defined as the sum of ticket time, the diff against Jira-logged time would be
  zero by construction, defeating the point of the feature.
- **A single `work_days` row per calendar date**, updated in place across clock-in/out
  cycles — rejected: would require deciding how to represent a lunch-time full
  clock-out (delete-and-recreate? a nullable "paused" state distinct from a break?)
  instead of just letting a new day naturally start a new row, which the existing
  single-open-row invariant already handles for free.
