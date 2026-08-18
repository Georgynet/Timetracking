# 0021: Statistics view — per-ticket totals and an interval breakdown chart

**Status:** Accepted — 2026-08-18

## Context

The spec (§4.7) always intended `time_entries` to be kept forever "as the foundation
for future statistics/reporting," but no view ever read that history in aggregate —
`HistoryList` only ever shows one day at a time. The user wanted two summary views:
total time per ticket over a range, and a per-interval (day/week/month) breakdown
showing both ticket allocation and break time, ideally as a stacked column chart.

`workday::engine` (ADR-0017/0018) already solved the hard parts of this exact
problem for the `WorkdayWidget` summary line: converting a local calendar range into
UTC bounds for querying `time_entries` (handling DST folds), and summing break time
across `work_days`/`work_breaks` with a live `now` standing in for anything still
open. This feature is a new read path over the same data, not a new data model.

## Decision

**Backend.** A new `stats::engine` module, mirroring the existing repo → engine →
command layering (ADR-0012), adds two pure functions:

- `ticket_totals(conn, from, to)` — sums `time_entries.duration_seconds` by
  `task_id` over an inclusive local-date range, sorted most-logged-first. Entries
  with no `duration_seconds` yet (the live running entry) are skipped, the same rule
  `range_summary` already uses for its "logged" total.
- `interval_stats(conn, from, to, granularity, now)` — buckets that same range into
  calendar-aligned day/week/month spans (week/month buckets align to
  `workday::engine::week_start`/`month_start` regardless of where `from` falls inside
  one) and, per bucket, returns per-ticket seconds plus total break time.

Rather than reimplement local-day → UTC-bounds conversion (with its DST-fold
handling) or break-summing, `workday::engine::local_range_bounds_utc` and
`break_seconds` were changed from private to `pub(crate)` and are called directly
from `stats::engine`. No schema change, no migration — this is a query-shape
addition over existing tables.

Two new commands, `get_ticket_stats` and `get_interval_stats`, follow the same
`*_impl` / `#[tauri::command]` split as every other command module.

**Frontend.** A new top-level view, `StatisticsView`, reached via a Tracker/
Statistics tab added to `HeaderBar` (state owned by `MainView`, so `App.tsx` and
`SetupView` stay untouched). It owns its own range (This Week / This Month / Custom
from–to) and Day/Week/Month granularity state locally, the same self-contained
fetch pattern `HistoryList` already uses rather than adding new global store state.
"This Week"/"This Month" are to-date (through today), consistent with the existing
week/month-to-date convention from ADR-0018.

**No new charting dependency.** `IntervalStatsChart` is a small hand-rolled inline-SVG
stacked column chart, styled with the app's existing plain-CSS variables rather than
pulling in a charting library for one chart. Its design follows the checked-in
`dataviz` skill:

- Tickets are ranked by total time across the whole chart (not per bucket) and the
  top 8 get a fixed categorical color slot from a validated 8-hue order (`--series-1`
  … `--series-8` in `App.css`); the same ticket always gets the same color and stack
  position in every column — color/position follow the ticket's identity, never its
  per-column rank. Any remaining tickets collapse into a single neutral "Other"
  segment; break time gets its own neutral "Break" segment. Both neutrals are
  desaturated grays, distinct from all eight hues and from each other by lightness.
- Stacked segments get a 2px surface-color gap between them, and the outermost
  (topmost) segment per column gets rounded top corners (4px) with a square baseline
  — the chart's only mark-anatomy rule beyond fill color.
- **Tooltips are a native SVG `<title>` per segment** (browser-native hover text),
  not a custom interactive tooltip layer — the simplest correct MVP for a small
  internal tool; the value is also always visible via the legend and the ticket
  table, so nothing is gated behind hover.
- A legend is always rendered (multiple series), with axis/legend text in the app's
  existing muted-ink tokens rather than the series colors themselves.

`TicketStatsTable` is a plain sorted table (ticket, summary, total duration) — no
chart, per the request that the ticket view "simply needs to show total time."

## Consequences

- Both views share one range control, so switching between "by ticket" and "by
  interval" never requires re-picking dates; the granularity toggle only affects the
  interval chart's bucketing.
- The 8-ticket-plus-"Other" cap keeps the chart legible for anyone who has logged
  time against dozens of tickets, at the cost of not distinguishing low-usage
  tickets from each other within "Other" (only the table view shows their exact
  totals).
- Making `local_range_bounds_utc`/`break_seconds` `pub(crate)` slightly widens
  `workday::engine`'s internal surface, but avoids a second, easy-to-drift
  implementation of DST-fold handling and live-break summing.
- Native `<title>` tooltips are a deliberately minimal interaction layer; a richer
  hover tooltip (exact value, keyboard-accessible) is easy to layer on later without
  changing the data shape.

## Alternatives considered

- **A charting library (e.g. Recharts)** — rejected: this app has no runtime
  dependency beyond `date-fns`/`zustand`/`react` so far, and one chart doesn't
  justify the bundle size and API surface of a full charting library.
- **One color per ticket, uncapped** — rejected: an unbounded, growing ticket set
  makes a legend (and the color-to-ticket mapping) unreadable past a handful of
  series; the validated categorical palette this codebase now uses is only
  guaranteed distinguishable up to 8 adjacent slots.
- **Re-sorting each column's stack by that column's own ranking** — rejected: color
  and position must track the ticket's identity, not its rank in a given column, or
  the same ticket would visibly jump color/position between columns, defeating the
  point of a legend.
