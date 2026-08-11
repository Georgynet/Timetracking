# 0015: Denormalize the ticket summary onto `TimeEntry` for the history tooltip

**Status:** Accepted — 2026-08-11

## Context

The History table shows each entry's Jira key (`taskKey`) but not its human-readable
ticket title, and the user asked for a tooltip on that cell showing the title.

The frontend already holds `Task` objects (which carry `summary`) in `MainView`, as
`allTasks = myTasks ∪ favoriteTasks`. Looking up a `TimeEntry`'s summary via
`allTasks.find(t => t.id === entry.taskId)` was the cheapest-looking option, since it
needs no backend change. That lookup is unsound, though: `allTasks` is only the tasks
currently assigned-to-me or favorited. A ticket tracked in the past that has since been
unfavorited and is no longer assigned to the user — a completed ticket, most commonly —
would drop out of `allTasks` while its `TimeEntry` rows remain in history forever. The
tooltip would silently go missing for exactly the entries most likely to be old enough
that the user actually needs the reminder.

## Decision

Add `task_summary` to the backend `TimeEntry` struct, sourced via the SQL join
`time_entries_repo` already performs against `tasks` (which previously selected only
`tasks.jira_key`, now also selects `tasks.summary`). This makes the summary a property
of the entry row itself, correct at any point in time regardless of the current
favorite/assigned-to-me set. The TS `TimeEntry` type gains `taskSummary: string`
correspondingly, and `HistoryList` sets it as the `title` attribute on the ticket-key
cell — a native browser tooltip, no new UI component needed.

## Consequences

- Every `TimeEntry`-returning path (`get_by_id`, `get_running`, `list_entries`,
  `list_pending_sync`, insert/update, all of which route through the shared
  `SELECT_JOIN` constant and `row_to_time_entry`) now carries `task_summary` for free,
  including places that don't currently render it (e.g. `ActiveTimer`, sync failures).
  That's a small amount of unused payload today in exchange for one join instead of N
  call sites each needing their own summary source.
- `task_summary` reflects whatever `tasks.summary` was as of read time, not the summary
  at the moment the entry was created. If a ticket's title changes in Jira and the
  local `tasks` row is later resynced, the tooltip on old entries updates to the new
  title rather than freezing to what it was when the time was logged. This is
  consistent with `taskKey`, which already behaves the same way (entries reference
  `task_id`, not a frozen key), so no new inconsistency is introduced.

## Alternatives considered

- **Client-side lookup against `allTasks`** — rejected: unsound for any ticket that has
  since fallen out of the assigned-to-me/favorites set, which is precisely the case
  where the user most wants the reminder.
- **Widen `allTasks` in `MainView` to include every task ever tracked** (e.g. a new
  `listAllTasks` fetching all rows in `tasks`) — would fix the lookup but pulls in a
  whole additional dataset and query path just to serve one string in one tooltip;
  denormalizing onto the row already being fetched is strictly less machinery for the
  same correctness.
