# 0033: Gate the workday sessions nav behind an off-by-default preference

**Status:** Accepted — 2026-09-26

## Context

ADR-0032 added a Prev/Next day nav row plus a sessions list row to `WorkdayWidget`, always
rendered, so a workday session from an earlier day could be found and corrected. In
practice this permanently costs two rows of vertical space in a widget that sits above
the timer and task panels, for a correction that's needed only occasionally (forgetting
to click "End Workday"). That's a worse every-day cost than the every-few-days benefit.

## Decision

Add a preferences toggle, off by default, that gates whether those two rows render at
all — same key/value preferences store as every other UI preference (ADR-0026), same
`get_preferences`/`save_preferences` round trip.

- `commands::preferences`: new `workday_editing_enabled: bool` field on `PreferencesDto`,
  stored under `ui.workday_editing_enabled`, defaulting to `false`.
- `SettingsModal`: a new "Workday" group with a single checkbox, "Allow editing past
  workday sessions".
- `WorkdayWidget`: the whole `.workday-sessions` block (both the nav row and the
  completed-sessions row) is now conditional on `workdayEditingEnabled` — off hides both
  rows entirely, on renders exactly what ADR-0032 shipped.

Nothing about the underlying `get_workday_sessions`/`update_workday` commands changes —
this is purely a rendering gate in the frontend, same as how the current sprint filter
(ADR-0024) or panel row counts are preferences without touching the commands they affect.
Turning the preference off doesn't lose data or block `EditWorkdayForm`/`EditBreakForm`
in any other way; it only stops the nav+list from being reachable through the widget
until switched back on.

## Consequences

- The common case (widget mostly idle, no forgotten clock-out) gets its two rows of
  space back by default.
- The occasional case (did forget) requires a trip to Settings first, once, to turn the
  feature on — a worse first-use experience than ADR-0032's always-on version, accepted
  as the tradeoff for not permanently taxing the common case.
- Same pattern as every other row-count/order/theme preference already in Settings, so
  no new plumbing shape — just one more boolean alongside `current_sprint_default`.

## Alternatives considered

- **Collapse the section instead of hiding it** (e.g. a "Show past sessions" toggle link
  inline in the widget) — rejected: still a permanent row of UI at rest, which is the
  exact cost this ADR is trying to avoid; a Settings toggle is truly zero cost when off.
- **Auto-detect and only show the rows when there's something to edit** (e.g. a workday
  ended unusually long ago) — more complex, and still doesn't address wanting the rows
  gone by default regardless of whether anything's currently wrong; deferred as
  unnecessary for what was asked.
