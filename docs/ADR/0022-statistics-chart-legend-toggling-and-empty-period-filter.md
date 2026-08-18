# 0022: Toggleable statistics-chart legend with an "Other" bulk-toggle; hide empty periods by default

**Status:** Accepted — 2026-08-18

**Supersedes, in part:** [0021](0021-statistics-view-ticket-and-interval-breakdown.md)'s
legend design (a plain, non-interactive "Other" entry with no way to see or affect
what it contains). ADR-0021's color-capping rationale and the rest of its decision
stand.

## Context

Using the statistics view surfaced two gaps:

1. `IntervalStatsChart`'s legend was static — there was no way to focus on one or
   two tickets/break time without the rest of the stack visually competing for
   attention.
2. There was no way to skip empty periods (e.g. weekends, or days with nothing
   logged) when scanning a longer range.

(An intermediate version of this change also listed every tail ticket individually
in the legend, to answer "what's actually inside Other." That turned out to be more
than needed — a single "Other" entry that can still be toggled, as described below,
covers the same need without a legend that keeps growing as more tickets accumulate.)

## Decision

**Click-to-toggle.** Every legend entry is now a button. The legend lists "Break",
then the top 8 tickets by total time in range (each with its own validated
categorical color, `--series-1`…`--series-8`, unchanged from ADR-0021), then a
single "Other" entry for everything beyond that. Clicking any of these toggles it
in and out of the stack for every column at once, and the y-axis rescales to
whatever's still visible — the standard "isolate a series" interaction most chart
libraries offer. A hidden entry stays in the legend, dimmed and struck through, so
it can be toggled back on.

**"Other" as a single bulk-toggle.** "Other" doesn't correspond to one ticket, so
clicking it doesn't hide a single segment — internally it toggles every
rank-9-and-beyond ticket at once (hides all of them if any is currently visible,
shows them all again once every one is hidden). Those tail tickets have no
individual entry of their own in the legend; "Other" is the only handle on them,
consistent with them sharing one neutral bar color rather than each getting a
distinct hue.

**Hide empty periods by default.** `StatisticsView` now filters `IntervalBucket`s
with no ticket time and no break time out of the chart by default, with a
"Show empty days/weeks/months" checkbox (label follows the active granularity) to
bring them back — the same checkbox-next-to-panel-header pattern `MyTasksPanel`
already uses for its "Current sprint" filter. This only affects the interval chart;
`TicketStatsTable` is unaffected since it has no per-period rows to begin with.

## Consequences

- The legend is now bounded at 10 entries (Break + 8 tickets + Other) regardless of
  how many distinct tickets appear in range, so it never grows unboundedly the way
  a fully-named legend would.
- Toggle state lives in `IntervalStatsChart`'s own local state and resets whenever
  its `buckets`/`granularity` props change (e.g. a new range is picked), rather than
  persisting across range changes — a fresh chart starts with everything visible,
  which is the least surprising default.
- The empty-period filter and the legend's show/hide toggles are independent and
  compose freely: hiding empty periods removes columns; hiding a ticket (or
  "Other") removes a segment from the remaining columns.
- A tail ticket can only be shown/hidden as part of the whole "Other" group, not on
  its own — an accepted trade-off for keeping the legend a fixed size; the exact
  breakdown of what's inside "Other" for a given period is still visible via the
  hover tooltip on that segment and via `TicketStatsTable`.

## Alternatives considered

- **List every tail ticket individually in the legend** (tried first) — reverted:
  it did answer "who's in Other," but at the cost of an unbounded, ever-growing
  legend for anyone who's logged time against many tickets, which was worse than
  the aggregate entry it replaced.
- **Keep the 8-ticket cap but make "Other" itself expandable** (click "Other" to
  see/toggle its members in a submenu) — rejected as more UI than the fixed-size
  legend needs; the tooltip and the ticket table already answer "who's in Other."
- **Persist hidden/shown state across range or granularity changes** — rejected: a
  ticket hidden in one range may not even appear in the next one, and carrying
  that state forward would need a stable per-ticket identity concern this view
  doesn't otherwise need to track.
