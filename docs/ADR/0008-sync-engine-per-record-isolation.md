# 0008: Sync engine — per-record failure isolation, chronological order, no retry queue

**Status:** Accepted — 2026-08-10

## Context

The spec requires that sync be manual (a single "Sync" button push), and that a
failure on one record (network error, invalid ticket, etc.) must not block the rest of
the batch — the failing record stays `is_synced = false` with a clear error shown, and
sync can be retried later with no data loss. This was left as an explicit open
technical question (#2: "behavior on partial sync failure — retry logic, order of
record submission").

## Decision

`sync::service::sync_all` loads all pending entries
(`WHERE ended_at IS NOT NULL AND is_synced = 0`) **ordered by `started_at` ascending**
and processes them one at a time, in-process, sequentially (not in parallel). For each:
on success, `mark_synced` is called immediately; on failure, the row is **left
completely untouched** and the error is recorded in the returned `SyncOutcome` — there
is no separate retry queue or backoff logic. Because the "pending" query is just "not
yet synced," clicking Sync again naturally re-selects exactly the rows that failed
last time, in the same order, achieving retry-with-no-data-loss for free rather than
through dedicated retry machinery.

Chronological order was chosen (over, say, reverse-chronological or unordered) so that
if a sync run is interrupted partway (app closed, network drops), the *earliest* gaps
in Jira's worklog history get filled first on the next attempt, rather than leaving an
arbitrary scattered set of gaps.

The DB mutex is only held for the brief read (listing pending entries) and the brief
write (marking one entry synced) around each request — never across the network
`.await` to Jira — so a slow or hanging sync doesn't block the rest of the app (timer
start/stop, manual entry edits) while it's in flight.

## Consequences

- A `SyncReportDto` (`total`, `succeeded: [entry_id]`, `failed: [{entry_id, task_key,
  message}]`) is returned to the frontend after every sync, which is enough to build
  both an immediate "N succeeded, M failed" summary and rely on the persistent
  per-row "pending" badge (already derived from `is_synced`) as the durable source of
  truth.
- There is no cap on how many times a failing record can be retried, and no
  exponential backoff — every Sync click is a fresh, unconditional attempt at every
  pending row. Acceptable given sync is manual and infrequent (a user clicking Sync
  repeatedly is the user's own choice, not a background loop that could hammer Jira).
- Sequential (not concurrent) requests to Jira per sync run — simpler failure
  semantics, and the batch sizes involved (a day's or week's worth of an individual's
  time entries) don't make this a meaningful throughput concern.

## Alternatives considered

- **Abort the whole batch on the first failure** — explicitly rejected by the spec
  itself ("the remaining records continue to sync").
- **A dedicated retry queue with backoff/attempt counters** — rejected as
  unnecessary complexity for a manually-triggered, low-volume, infrequent operation;
  "click Sync again" already achieves the same outcome.
