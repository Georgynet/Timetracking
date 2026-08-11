# 0009: Allow hard-delete only for manual entries that were never synced

**Status:** Accepted — 2026-08-10

## Context

The spec states time records are "never deleted" and are kept indefinitely, synced or
not, as the foundation for future statistics. It does not, however, provide any way to
remove a record at all — including a plainly wrong one, such as a manual entry created
against the wrong ticket by mistake and never touched Jira. Without *some* corrective
mechanism, such a mistake is permanent.

## Decision

`commands::entries::delete_draft_entry` hard-deletes a `time_entries` row, but only
when **all** of these hold: `created_manually = true`, `is_synced = false`, and
`jira_worklog_id IS NULL`. Any row that was ever created by the timer, or that has
ever been pushed to Jira (even if later edited back to `is_synced = false` per
ADR-0007), is rejected with a validation error and cannot be deleted through this
command. The frontend (`HistoryList`) mirrors this same check client-side to hide the
Delete button entirely for ineligible rows, rather than only surfacing the rejection
after a failed attempt.

This is a **deliberate, narrow deviation** from a literal reading of "never deleted" —
scoped tightly enough that it cannot touch anything the spec's "keep forever for
future statistics" intent actually protects (nothing timer-tracked, nothing that ever
reached Jira).

## Consequences

- A fat-fingered manual entry (wrong ticket, typo'd duration, entered on the wrong
  day) can be removed outright instead of requiring the user to edit it into a
  harmless-but-permanent zero-duration ghost row.
- Any entry that reaches Jira even once becomes permanent in the local DB for the
  rest of the app's life, matching the spec's stated intent exactly.
- If future reporting/statistics work (explicitly out of scope for this MVP) ever
  wants to count *all* manual entries ever created, including deleted drafts, this
  decision would undercount — accepted as an explicit trade-off in favor of letting
  users fix mistakes now.

## Alternatives considered

- **No delete capability at all** — the spec-literal option; rejected as leaving no
  way to correct an obvious, harmless mistake before it ever reaches Jira.
- **Soft-delete (an `is_deleted` flag, filtered out of the UI)** — would avoid ever
  removing a row, but adds a new column/filter to thread through every query for a
  case (draft mistakes) that carries no statistical value to preserve; rejected as
  unnecessary complexity for this MVP.
