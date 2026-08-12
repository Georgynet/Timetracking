# 0019: "Current sprint" filter for My Tasks via a second best-effort JQL query

**Status:** Accepted — 2026-08-12

## Context

The user wanted to narrow the "My Tasks" list down to tickets in their currently
active sprint, without losing the existing "all assigned, unresolved tickets" view —
a toggle between the two, not a replacement.

Jira Cloud has two ways to get at sprint membership: the Agile REST API
(`/rest/agile/1.0/board/{id}/sprint`), which needs a board ID and returns rich sprint
metadata (start/end dates, state, board association), or JQL's built-in
`sprint in openSprints()` function, which works directly against the same
`/rest/api/3/search/jql` endpoint `JiraClient::search_issues` already calls. The
Agile API also requires knowing which board a ticket belongs to, which this app has
no concept of and would need a separate lookup to establish. Since the only thing the
UI needs is a yes/no per ticket ("is this in an active sprint"), not sprint metadata,
JQL is the much smaller addition.

Not every Jira project uses Scrum boards/sprints at all — for a project with no
`sprint` field, `sprint in openSprints()` is a JQL error, not an empty result.

## Decision

`commands::tasks::refresh_my_tasks` now runs a second JQL query alongside the existing
"my tasks" one: `assignee = currentUser() AND resolution = Unresolved AND sprint in
openSprints()`. No `JiraClient` trait change was needed — this reuses
`search_issues(jql, max_results)` with a different query string. The returned keys are
collected into a set and used to set a new `is_in_current_sprint` flag when upserting
each task (`tasks_repo::upsert_assigned_task`, extended with an `is_in_current_sprint:
bool` parameter), backed by a new column added via `migrations/003_task_sprint_flag.sql`
(`ALTER TABLE tasks ADD COLUMN is_in_current_sprint INTEGER NOT NULL DEFAULT 0`) — the
same additive, non-destructive migration style as `002_work_days.sql`.

The sprint query is treated as **best-effort**: `commands::tasks::current_sprint_keys`
catches any error from it (e.g. the JQL error a non-Scrum project throws) and falls
back to "nothing is in the current sprint" (logged via `tracing::warn!`, safe to log
since `JiraError`'s `Display` never embeds request headers or secrets — see ADR-0011)
rather than failing the whole "My Tasks" refresh. A user on a Kanban-only or
non-agile project still gets their assigned-tickets list; they just never see anything
flip to "in current sprint" — which is the correct behavior, not a degraded one, since
that's genuinely true for them.

`is_in_current_sprint` is reset alongside `is_assigned_to_me` in
`tasks_repo::reset_assigned_to_me` (unlike `is_favorite`, which is deliberately
independent per ADR — see the existing comment on that function) — a ticket's sprint
membership is only ever meaningful in the context of "my tasks," so it's cleared and
re-populated on the same refresh cycle rather than persisting stale across refreshes.

On the frontend, `MyTasksPanel` filters the already-fetched `tasks` array client-side
based on `isInCurrentSprint` via a plain checkbox toggle — no new API call, no new
store action. This is a pure display concern (matching the "frontend is a thin
view/state layer" architecture note): the toggle only ever re-slices data that a
`Refresh` click already fetched and persisted with the correct flag.

## Consequences

- No `JiraClient` trait change, no Agile REST API usage, no board-ID lookup — this is
  the smallest change that satisfies "filter my tasks by current sprint."
- `refresh_my_tasks` now makes two Jira requests instead of one. Both are small
  (`maxResults: 100`) JQL searches against the same endpoint, so the added latency is
  minor and bounded.
- Toggling the filter is instant and offline-safe (no network round-trip) once a
  refresh has populated the flag — but it's only ever as fresh as the last `Refresh`
  click, same as the rest of "My Tasks."
- `FakeJiraClient::search_issues` special-cases any JQL containing `openSprints()` to
  return a separate `sprint_search_results` field instead of the general
  `search_results` — the fake still doesn't parse JQL, but this is enough to let tests
  simulate "some of my tasks are in the sprint, some aren't" and "the sprint lookup
  errors out" independently of the main list.

## Alternatives considered

- **Agile REST API** (`/rest/agile/1.0/board/{id}/sprint/{sprintId}/issue`) — rejected:
  requires discovering the board ID first (a lookup this app has no existing concept
  of), and returns far more than the single boolean the UI needs.
- **Requesting the sprint custom field** (typically `customfield_10020`, but not
  guaranteed stable across Jira sites) on the existing search and parsing its JSON
  shape client-side — rejected: brittle across Jira instances with a different custom
  field ID for Sprint, and still requires deciding which of possibly-multiple sprints
  on an issue counts as "current," which `sprint in openSprints()` already resolves
  correctly server-side.
- **Re-querying Jira on every toggle flip** instead of persisting the flag — rejected:
  adds a network round-trip to what should be an instant, purely-local UI filter, and
  duplicates state that's already known right after a refresh.
