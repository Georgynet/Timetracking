/// Initializes `tracing` for the app (pretty logs to stdout).
///
/// Conventions used throughout this codebase to guarantee secrets never reach a log:
/// 1. The `settings` table/struct never has a token field — only `jira_base_url` and
///    `jira_email` — so even an accidental `{:?}` log of it is safe by construction.
/// 2. The API token lives only in the OS keychain (`secrets::keyring_store`); it is
///    never placed into any struct that could plausibly be logged.
/// 3. `jira::models::JiraError` is only ever built from HTTP status codes and response
///    bodies, never from request headers — see the regression test
///    `jira::models::tests::jira_error_variants_never_embed_request_headers`.
/// 4. Never log a whole request/response object with `{:?}`; log individually-chosen
///    primitive fields (method, path, status, ids, counts) instead.
pub fn init() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
}
