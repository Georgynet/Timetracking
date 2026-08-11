# 0013: The running timer cannot be edited or deleted outside start/stop

**Status:** Accepted — 2026-08-11

## Context

ADR-0006 establishes the running timer as a `time_entries` row with `ended_at IS
NULL`, with `timer::engine::start`/`stop` as the only code paths meant to transition it
into a completed row. `commands::entries::update_time_entry`, however, had no check
against editing *that specific row* through the ordinary manual-edit form — the
History list showed an Edit button for every entry, including the running one.

This was a real bug, not just a theoretical gap: editing the running entry's start/end
time from History set `ended_at` to a real timestamp directly, which silently took the
row out of "running" state without going through `timer::engine::stop`. The frontend's
cached `activeTimer` (last fetched at launch, or after the last successful
start/stop) was never invalidated by this edit, so the Timer widget kept rendering the
old, now-defunct entry as running. Clicking Stop then called the backend's `stop_timer`
command, which found no row with `ended_at IS NULL` and returned a "no timer is
currently running" error — which `TimerWidget`'s stop handler had no `catch` for, so
it surfaced as nothing at all. The result: a timer permanently stuck showing
"running" with a Stop button that failed silently on every click, recoverable only by
directly editing the database.

## Decision

The running entry's lifecycle is owned exclusively by `timer::engine` — `update_time_entry`
and `delete_draft_entry` must never be able to touch it:

- **Backend**: `commands::entries::update_time_entry_impl` now checks
  `current.is_running()` first and rejects the edit with a validation error
  ("Cannot edit the currently running timer — stop it first.") before doing anything
  else. `delete_draft_entry` already rejected it correctly as a side effect of its
  existing guard (`created_manually` is always `false` for timer-created rows — see
  ADR-0009) but that was incidental, not an explicit intent; this ADR makes the intent
  explicit for future readers.
- **Frontend**: `HistoryList` hides the Edit button entirely for the row where
  `endedAt === null`, showing a "Running" status badge instead of Synced/Pending, so
  the now-rejected action is never offered as a UI affordance in the first place
  rather than only failing after the fact.
- **Defense in depth against the same class of bug recurring**: `TimerWidget`'s
  start/stop handlers now catch and display errors instead of swallowing them, and
  `store.ts`'s `startTimer`/`stopTimer` always refetch `activeTimer` from the backend
  in a `finally` block — even on failure — so a stale cached timer state can no longer
  get the UI permanently stuck; a failed Stop now self-corrects on the next render
  instead of failing identically forever. `MainView` also refreshes `activeTimer`
  whenever *any* history entry changes, as further insurance.

## Consequences

- There is now no way to change the ticket, start time, or duration of a running
  timer without first stopping it — matching the user-facing expectation that a
  timer's lifecycle is: start → (optionally) stop-and-becomes-editable. To fix a
  running timer's ticket (the one case the spec calls out: "changing the ticket a
  record is linked to in case the user forgot to switch the timer"), the timer must
  be stopped first, then edited as a normal completed entry.
- The frontend/backend guard is duplicated by design (UI hides the affordance, backend
  independently rejects it) rather than trusting the UI alone — the backend check is
  the actual source of truth and is what the regression test
  (`commands::entries::tests::editing_the_running_timer_is_rejected`) verifies.
- The store-level "always refetch on failure, even in a `finally`" pattern is now the
  precedent for any future mutation that touches the active timer indirectly — a
  narrower fix (just guarding the one edit path) would have closed this specific bug
  but left the underlying "a rejected mutation can leave cached state silently stale"
  failure mode available to the next such case.

## Alternatives considered

- **Allow editing the running entry's ticket/comment, but not its start/end time** —
  considered, since the spec's "forgot to switch the timer" scenario is specifically
  about the ticket, not the times. Rejected as more complex to implement correctly
  (a partial-field guard rather than a single running-check) for a case the user can
  already handle by stopping the timer, fixing the ticket via a normal edit, and
  starting a new one if they want to keep tracking — simpler, and consistent with
  "active tasks shouldn't be editable" as stated by the user who reported this bug.
