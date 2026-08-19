# 0023: Search Jira directly from the manual-entry form for one-off tickets

**Status:** Accepted — 2026-08-19

## Context

The user wanted to log time (via "New manual entry") against a ticket that is neither
assigned to them nor already favorited — the recurring example being an ad-hoc code
review on someone else's ticket. Before this change, `ManualEntryForm`'s ticket
`<select>` only offered `allTasks` (My Tasks ∪ Favorites, as assembled in
`MainView.tsx`), so logging against any other ticket required first switching to the
Favorites panel, searching for and adding the ticket there, then returning to the
manual-entry form to find it in the now-updated dropdown.

Every time entry syncs to Jira as a worklog against a real ticket (`time_entries.task_id`
→ `tasks.jira_key`), so there was never a question of allowing a free-text "ticket name"
— the entry must resolve to an actual Jira issue, and that issue must land in the local
`tasks` table to satisfy the foreign key. `FavoritesPanel`'s existing `search_jira_issues`
command (exact-key lookup or free-text JQL) already solves the lookup half of this. Its
`add_favorite_by_key` command solves the DB-write half, but does so by permanently
marking the ticket `is_favorite = true` — the user explicitly did not want that: a
one-off ticket like a single code review shouldn't clutter the Favorites list
afterwards, unlike the genuinely-recurring "container" tickets (`TEAM-1`, etc.) Favorites
is meant for per spec §4.3.

## Decision

- Added a new repo function `tasks_repo::upsert_task` and command
  `resolve_task_by_key`, parallel to `upsert_favorite_task`/`add_favorite_by_key` but
  never setting `is_favorite` (or `is_assigned_to_me`) — it only inserts/refreshes the
  `tasks` row so a time entry can reference it. Re-running it against an
  already-favorited ticket leaves the favorite flag untouched (tested), so it's also
  safe to accidentally use on a ticket that happens to already be a favorite.
- `ManualEntryForm` now has a collapsed-by-default "Ticket not in the list? Search Jira
  for a one-off ticket…" affordance — same search-box-then-results UX as
  `FavoritesPanel` — reusing `search_jira_issues` for the lookup and calling
  `resolve_task_by_key` (not `add_favorite_by_key`) when a result is picked.
- The resolved task is kept only in the form's own local state (`foundTasks`), merged
  into the ticket dropdown for the lifetime of that dialog. Nothing propagates to the
  Zustand store's `myTasks`/`favoriteTasks` — a one-off ticket is deliberately not meant
  to reappear anywhere else in the UI on its own.
- Editing an existing entry that references such a one-off ticket seeds `foundTasks`
  from the entry's own denormalized `taskKey`/`taskSummary` (`taskFromEntry`), so the
  dropdown still shows it correctly even though it's absent from `tasks`/`favoriteTasks`.

## Consequences

- A one-off ticket can be logged against from the manual-entry form in one dialog,
  without a detour through Favorites and without permanently polluting it.
- The ticket's `tasks` row persists indefinitely (per the "records are never deleted"
  rule), so it still shows correctly in History and Statistics — it just won't resurface
  in My Tasks, Favorites, or a future manual entry's dropdown without searching again.
  That repeat-search cost is the intended trade-off for "one-time tracking."
- Two near-identical repo functions/commands now exist
  (`upsert_task`/`resolve_task_by_key` vs. `upsert_favorite_task`/`add_favorite_by_key`)
  differing only in whether `is_favorite` is set — kept separate rather than adding a
  boolean flag, since the two call sites (Favorites panel vs. manual-entry one-off
  search) have deliberately different persistence semantics.

## Alternatives considered

- **Reuse `add_favorite_by_key` and let the user manually un-favorite it afterwards**
  (the first version of this change) — rejected per explicit user feedback: a one-off
  ticket shouldn't have to be saved to Favorites at all, even transiently.
- **Allow a plain free-text "ticket" with no Jira resolution** — rejected: breaks the
  sync model outright (§4.6 requires a real `jira_key` to `POST`/`PUT` a worklog against).
- **Don't persist the resolved ticket in `tasks` at all, only in form state** — not
  possible: `time_entries.task_id` is a foreign key into `tasks`, so the row must exist
  before the entry can be saved.
