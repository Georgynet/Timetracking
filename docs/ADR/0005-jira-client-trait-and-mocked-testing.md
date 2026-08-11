# 0005: Abstract Jira access behind a trait; verify via mocks, defer live Jira testing

**Status:** Accepted — 2026-08-10

## Context

No live Jira Cloud instance or API token was available during this implementation
session. The app still needs its Jira integration (auth, task search, worklog
create/update) to be exercised by automated tests, and the business logic that sits on
top of it (timer engine, sync engine) needs to be testable without any HTTP
involved at all.

## Decision

Define a `JiraClient` trait (`jira::client_trait`) with one method per Jira operation
the app needs (`get_myself`, `search_issues`, `get_issue`, `add_worklog`,
`update_worklog`). Two implementations exist:

- `jira::reqwest_client::ReqwestJiraClient` — the real implementation, used at
  runtime, backed by `reqwest`.
- `jira::fake_client::FakeJiraClient` — an in-memory fake with configurable canned
  results per issue key and a call log, used by `timer`/`sync` unit tests with no
  network involved.

The real client is additionally covered by `tests/jira_client_tests.rs`, which spins
up a local mock HTTP server via `wiremock` and asserts the real `ReqwestJiraClient`
hits the correct paths, headers, and request bodies — notably the current
`POST /rest/api/3/search/jql` endpoint rather than the deprecated
`GET /rest/api/3/search`, and that worklog comments are wrapped in Atlassian Document
Format. This is the closest verification possible without live credentials, but it
only proves the client sends what we believe Jira expects — not that Jira actually
accepts it.

## Consequences

- Business logic (`sync::service::sync_all`, timer engine interactions with Jira) is
  fully unit-tested against `FakeJiraClient`, including "one entry fails, the rest of
  the batch still succeeds" — see ADR-0008.
- A real end-to-end pass against a live Jira Cloud instance (real `GET /myself`, real
  JQL pagination beyond the first page, a real worklog POST/PUT round trip, and
  confirming Jira actually accepts the `started` timestamp format and ADF comment
  shape used) is **explicitly deferred** to whoever first configures the app with real
  credentials. This is called out in `README.md`'s "Known gaps" section.
- JQL search only requests the first page of results (`maxResults: 100`, no
  `nextPageToken` follow-up) — acceptable for "my assigned unresolved tickets" at
  individual-developer scale, but a real account with >100 matching tickets would
  silently see only the first page. Flagged here rather than solved speculatively.

## Alternatives considered

- **Skip Jira tests entirely, only test manually once credentials exist** — rejected;
  would leave the sync/timer business logic (the highest-risk part of the app —
  data loss, duplicate worklogs) without any automated coverage until then.
