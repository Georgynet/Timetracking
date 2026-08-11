# 0011: Guarantee log safety by construction, not a redaction layer

**Status:** Accepted — 2026-08-10

## Context

The spec requires that API tokens and other sensitive data never appear in
application logs. The obvious general-purpose solution is a redacting logging
middleware or a custom `reqwest` layer that scrubs `Authorization` headers before
anything is written out.

## Decision

Instead of a redaction layer, secrets are kept **structurally incapable of being
logged** in the first place:

1. The `settings` SQLite table and its Rust struct (`db::models::SettingsRow`) never
   have a token field at all — only `jira_base_url`/`jira_email` — so even an
   accidental `{:?}` log of the whole struct is safe. The token exists only in the OS
   keychain (ADR-0004) and is never placed into any struct that could plausibly be
   logged.
2. `jira::models::JiraError` — the only Jira-related error type that ever reaches a
   log or the frontend — is built **exclusively** from HTTP status codes and response
   bodies (see `jira::reqwest_client::ReqwestJiraClient::handle_response`), never from
   the request itself. It has no code path that could embed the `Authorization`
   header or the raw token, by construction, not by filtering.
3. A regression test
   (`jira::models::tests::jira_error_variants_never_embed_request_headers`)
   constructs every `JiraError` variant and asserts none of their `Display`/`Debug`
   output contains `"authorization"` or `"basic "` — a cheap tripwire against a future
   change accidentally threading request data into an error variant.
4. As a documented convention (top of `logging.rs`): never log a whole
   request/response/settings object with `{:?}`; log individually-chosen primitive
   fields (method, path, status, ids, counts) instead.

`tracing` + `tracing-subscriber` is used for structured logging, with no custom
`reqwest` middleware layer at all.

## Consequences

- There is no single place to audit for "does this scrub secrets correctly" — safety
  is distributed across "this struct has no token field" and "this error type has no
  code path that touches headers," which is easy to verify by reading each type's
  definition but relies on future changes preserving the same discipline rather than
  a middleware catching a mistake centrally.
- No performance or complexity cost from wrapping every HTTP call in a
  logging/redaction layer.
- If a future contributor adds a new error type or struct that *does* carry
  request-level data (e.g. for richer diagnostics), the burden is on them to keep it
  out of anything `Debug`/`Display`-logged — this ADR is the reference for why that
  matters here.

## Alternatives considered

- **A custom `reqwest` middleware that redacts the `Authorization` header before
  logging requests** — rejected: adds real complexity (a tower/reqwest-middleware
  layer) to guard against a mistake ("logging a whole request object") that's simpler
  to just not make possible in the first place, given the request/error types
  involved are small and fully under this app's control.
