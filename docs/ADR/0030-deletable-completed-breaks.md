# 0030: Allow deleting a completed break

**Status:** Accepted — 2026-09-02

## Context

ADR-0020 added an edit path for a completed break's start/end time, but no way to
remove a break outright. In practice, "Start Break" is sometimes clicked by mistake —
there was no incorrect *duration* to fix, the break itself shouldn't have been
recorded at all. Editing bounds can't express "this row shouldn't exist"; the only
workaround was living with a bogus break skewing `worked_seconds` and the
worked-vs-logged diff for the day.

## Decision

Add `delete_break`, following the same repo → engine → command → frontend layering as
`update_break`, and reusing its precondition: **a currently-running break cannot be
deleted** — its lifecycle stays owned exclusively by `start_break`/`end_break` — so
removing a break started by mistake is the same two-step flow ADR-0020 already
established for correcting one: stop it (however soon after), then delete it. This
mirrors `time_entries`' own rule that the running timer is never directly deletable
(ADR-0013), rather than inventing a new "cancel while running" affordance.

New pieces:

- `db::work_days_repo::delete_break(conn, id)` — a plain `DELETE`.
- `workday::engine::delete_break(conn, id)` — the break must exist and must not
  currently be running (`WorkdayError::CannotDeleteRunningBreak`), otherwise defers to
  the repo.
- `commands::workday::delete_break` — thin Tauri wrapper, no request body beyond the id.
- Frontend: `EditBreakForm` (opened by clicking a completed break in `WorkdayWidget`,
  same entry point as editing) gained a "Delete" button next to Save/Cancel, gated
  behind the same `confirm()` dialog pattern `HistoryList` already uses for deleting a
  time entry.

Unlike `time_entries` (ADR-0009/0014), there's no sync-status gate to add — breaks
carry no sync-related columns at all (ADR-0017), so "not currently running" remains
the only precondition, same as for editing.

## Consequences

- Removing a mistaken break still takes two actions (End Break, then Delete) instead
  of one "cancel" click, but keeps `start_break`/`end_break` as the only code path that
  changes whether a break is running — consistent with ADR-0020's edit-gating
  rationale and ADR-0013's rule for the timer.
- Deleting a break silently changes `worked_seconds` for whatever day it belonged to
  (the deleted span reverts to counting as worked time). No confirmation beyond the
  existing delete dialog is needed since, unlike a time entry, a break was never
  something synced anywhere.
- Same reachability gap as ADR-0020: only breaks under the currently active workday
  are visible to delete, since `WorkdayWidget` only renders `activeWorkday.breaks`.

## Alternatives considered

- **Let deleting a running break implicitly close it first** (skip the "stop it
  first" step) — rejected: would introduce a second path (alongside `end_break`) that
  transitions a break from open to closed, the exact thing ADR-0020 avoided when
  designing the edit gate.
- **Soft-delete / hide instead of hard-delete** — rejected: there's no sync state or
  history view that would ever need to distinguish a deleted break from one that never
  existed, so a hard `DELETE` is simpler and matches ADR-0009's precedent for
  never-synced `time_entries`.
