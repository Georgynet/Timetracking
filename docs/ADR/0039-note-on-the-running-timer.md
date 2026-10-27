# 0039: A note on the running timer, saved as the entry's comment while typing

**Status:** Accepted — 2026-10-27

## Context

A worklog comment could only be set when starting a timer (the "Comment (optional)"
field in the start form) or afterwards, by editing the stopped entry in History. What
was actually done is usually known only while working, so notes ended up either
missing or added later from memory. Once the timer runs, the start form is replaced by
the ticket key, the elapsed time and Stop — there was nowhere to write.

## Decision

The running timer row gets a "Comment (optional)" field between the elapsed time and
Stop. It shows and edits the `comment` of the running entry itself — the same field the
start form fills and the History edit form changes, and the one sync sends to Jira.

- The note is saved while typing (debounced), on blur / Enter, and once more right
  before Stop. It is not held in the frontend until Stop, because the timer can also
  be stopped from the tray or by switching tickets; since the note is already on the
  entry, every stop path keeps it.
- New command `update_running_comment(id, comment)` backed by
  `time_entries_repo::set_running_comment`. It trims the text (empty → no comment)
  and does not touch `edited_at` or `is_synced`: a running entry has never been
  synced, and a live note is not an after-the-fact edit.
- This narrows ADR-0013 for the comment only. ADR-0013 keeps the running entry's
  *lifecycle* (ticket, start, end, duration) owned by `timer::engine`; the comment
  doesn't affect it, and `update_time_entry` still rejects the running entry.
- The command takes the entry `id` and is rejected unless that entry is still the
  running one, so a late save (typing just before a tray switch) can't land on the
  newly started entry.

## Consequences

- Comments can be written while working and reach Jira through the normal sync.
- The field is reset from the entry whenever a different timer becomes active, but
  not on refetches of the same timer, so typing isn't overwritten.
- A note typed in the last moment (< the debounce delay) before the timer is stopped
  from the tray is dropped; stopping from the window always saves it first.

## Alternatives considered

- **Pass the note to `stop_timer`** — rejected: the tray and ticket switching stop the
  timer without the window, so the note would be lost on those paths.
- **Allow `update_time_entry` on the running entry** — rejected: it also changes
  start/end/duration and stamps `edited_at`, and its guard against the running entry
  (ADR-0013) is deliberate.
