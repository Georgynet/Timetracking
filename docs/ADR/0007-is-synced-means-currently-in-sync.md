# 0007: Redefine `is_synced` as "currently in sync" to support worklog updates without a schema change

**Status:** Accepted — 2026-08-10

## Context

Per the spec, editing a time entry that has already been synced to Jira must, on the
next sync, **update** the existing Jira worklog (`PUT /issue/{key}/worklog/{id}`)
rather than create a duplicate (`POST /issue/{key}/worklog`). This was left as an
explicit open technical question (#1: "exact mechanism for updating a worklog when an
already-synced record is edited"). The schema (per spec) has `is_synced` (boolean) and
`jira_worklog_id` (nullable) on `time_entries`, and no field was added.

## Decision

`is_synced` means **"this row's current data matches what's in Jira"**, not
**"this row has ever been pushed to Jira."** `db::time_entries_repo::update_entry`
resets `is_synced = 0` whenever an entry with a non-null `jira_worklog_id` is edited
(the worklog id itself is left untouched). The combination this produces —
`jira_worklog_id IS NOT NULL AND is_synced = 0` — is exactly the "needs a PUT, not a
POST" signal `sync::service::sync_all` reads: `jira_worklog_id.is_some()` routes to
`update_worklog`, `None` routes to `add_worklog`. The "entries needing sync" query is
simply `WHERE ended_at IS NOT NULL AND is_synced = 0`, with no separate "dirty" flag or
schema change required.

`edited_at` is stamped on every edit regardless of sync state, but purely for
UI/audit display (an "edited" badge) — it plays no role in the sync decision itself.

## Consequences

- No migration was needed beyond the spec's original schema to support update-vs-create.
- Editing a *never-synced* entry is a no-op with respect to `is_synced` (it was
  already `0`) — the resync path only actually changes behavior for entries that had
  previously succeeded.
- Anyone reading `is_synced` in isolation should not assume "0 = never synced" — it
  can also mean "was synced once, then edited, and is now stale relative to Jira."
  `jira_worklog_id`'s presence is what distinguishes the two, and this ADR is the
  reference for that distinction.

## Alternatives considered

- **Add a new `needs_resync` boolean column** — rejected: would duplicate information
  already derivable from `is_synced` + `jira_worklog_id`, and would need its own
  migration and its own invariant to keep in sync with `is_synced` going forward.
