# 0026: Ticket pickers rank by what you last tracked

**Status:** Accepted — 2026-08-21

## Context

Every ticket picker listed tickets in `jira_key` order, which is the order the repo
queries return and carries no meaning for the person choosing: `CTVSP-1499` is not
more likely to be what you want than `CTVSP-9275`. In practice a day's work cycles
between a handful of tickets, and the next timer is nearly always one of the ones
already tracked recently — information the app has locally and was ignoring.

## Decision

- `Task` gains `last_tracked_at`: the newest `time_entries.started_at` filed against
  it, added to the repo's `SELECT` as a correlated subquery. It counts manual entries
  as well as timer ones — both mean "I worked on this".
- `lib/tasks.ts::orderTasks` sorts by it, newest first, with never-tracked tickets
  after them in key order so the tail is predictable rather than arbitrary. It's
  applied to the combined list `MainView` hands the pickers, so the timer and both
  entry dialogs stay consistent with each other.
- The order is a preference (`ui.ticket_order`, ADR-0025): `recent` (default) or
  `key`. An unknown stored value falls back to the default rather than failing.

## Consequences

- The picker's contents now move between sessions. That is the point — but it means
  muscle memory for "the third one down" is worth less than it was, which is part of
  why the alphabetical order stays available as a setting rather than being replaced.
- `last_tracked_at` is computed per read rather than stored. At this table size
  (tens of tickets, thousands of entries) the subquery is free, and there is no
  denormalised column to keep in step with entry edits and deletes.
- The task panels (My Tasks, Favorites) keep their key order. They are browsing lists
  with their own semantics — Favorites in particular is a list the user arranged — and
  reordering them under the user would be a different decision from ranking a
  search-and-pick control.

## Alternatives considered

- **Rank by how often a ticket is tracked, not how recently** — favours long-running
  tickets over the one picked up an hour ago; recency matches the "what am I on today"
  question the picker is asked.
- **A separate "recent" section pinned above the full list** — clearer, but the picker
  is already filtered by typing, and a section header inside a filtered list is
  awkward to keep coherent as the query narrows.
- **Store `last_tracked_at` on the row, updated on start** — avoids the subquery at the
  cost of a column that silently goes stale whenever an entry is edited or deleted.
