# 0014: Sync status gates edit/delete — unsynced is fully mutable, synced is permanent

**Status:** Accepted — 2026-08-11

## Context

The original spec (section 4.5) required that editing an already-synced record
(`is_synced = true`) be supported, with re-synchronization correctly sending a
`PUT`/update to the existing Jira worklog rather than creating a duplicate — ADR-0007
implemented exactly this, by having `time_entries_repo::update_entry` reset
`is_synced` back to `false` when editing a row that already carries a
`jira_worklog_id`. Deletion was, from the start, deliberately narrow (ADR-0009):
allowed only for manually-created entries that had never been synced.

The user who owns this project then gave two direct product instructions that
partially reverse that starting point:

> The unsynced history items must be deletable. Synced history items shouldn't be
> editable.

Taken literally and to their conclusion, these two rules **remove the "edit a synced
entry, it becomes pending again" pathway entirely** — if a synced entry can never be
edited in the first place, it can never re-enter the "pending, but already has a
worklog" state ADR-0007 was built to handle. This is a deliberate simplification of
the spec's original 4.5 requirement, not an oversight: the user is choosing "synced =
permanent, fix mistakes in Jira directly" over "synced entries can be corrected from
the app and will re-sync."

## Decision

- `commands::entries::update_time_entry` now rejects editing any entry where
  `is_synced = true`, in addition to the pre-existing rejection of the running timer
  (ADR-0013). The error is a plain validation message: "Cannot edit an entry that has
  already been synced to Jira."
- `commands::entries::delete_draft_entry` now allows deleting **any** completed entry
  with `is_synced = false` and no `jira_worklog_id` — the `created_manually`
  requirement from ADR-0009 is dropped, so a timer-created entry that hasn't been
  synced yet is deletable exactly like a manual one. The running timer, previously
  protected *incidentally* by the `created_manually` check (timer rows always have
  `created_manually = false`), is now protected by an **explicit** `is_running()`
  check, since removing that requirement would otherwise have accidentally made the
  running timer deletable.
- The frontend (`HistoryList`) mirrors both rules client-side (`canEdit`/`canDelete`)
  so the now-invalid actions are never offered as buttons in the first place, rather
  than only failing after a click.
- `time_entries_repo::update_entry`'s resync-derivation logic from ADR-0007 (resetting
  `is_synced` to `false` when a row with a `jira_worklog_id` is edited) is
  **deliberately left in place at the repository layer**, even though the command
  layer above it can no longer reach that condition (a synced entry can never get to
  `update_entry` in the first place). It remains correct, tested, general-purpose
  data-layer behavior — removing it would mean re-adding it later would require
  re-deriving the exact same logic if this policy is ever relaxed, for no benefit
  today. The state it exists to handle is simply unreachable through the app's current
  command surface, not incorrect to keep supporting at the data layer.

## Consequences

- The state space for a completed entry is now exactly two states in practice:
  **pending** (`is_synced = false`, fully mutable — editable and deletable,
  regardless of how it was created) and **synced** (`is_synced = true`, fully frozen —
  neither editable nor deletable). The intermediate "was synced, edited back to
  pending, still carries a worklog id" state from ADR-0007 can no longer be *created*
  through the app going forward (only edits reach it, and edits on synced rows are now
  blocked) — though the guard against deleting such a row if it somehow exists (e.g.
  data from before this change) is kept for safety.
- This directly reverses spec section 4.5's requirement that an already-synced record
  remain editable with correct re-sync — recorded here explicitly as an intentional,
  user-directed deviation from the original spec text, not a bug.
- Fixing a mistake in a synced entry (wrong duration, wrong ticket, typo in the
  comment) now has no in-app remedy — it must be corrected directly in Jira. This is
  an accepted trade-off in favor of a simpler mental model and removes the
  edit-then-resync code path as a source of complexity/risk (e.g. a worklog getting
  updated with unintended values, or update/create ambiguity) from the app's actual
  behavior, even though the underlying mechanism remains available at the data layer
  if this is revisited.

## Alternatives considered

- **Keep editing allowed for synced entries, exactly as ADR-0007 designed** — rejected
  per explicit user instruction.
- **Remove the resync-reset logic from `time_entries_repo::update_entry` entirely**,
  treating ADR-0007 as fully superseded — considered, but rejected: it is still
  correct, still tested, general data-layer behavior with no cost to keeping it, and
  ripping it out would only need to be redone if this product decision is ever
  revisited (e.g. an admin/support tool for fixing synced entries).
