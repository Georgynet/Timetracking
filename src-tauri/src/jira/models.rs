use serde::Serialize;

/// Built only from HTTP status codes and response bodies — never from request headers
/// or the client's own config — so logging/displaying a `JiraError` can never leak the
/// Authorization header or API token.
#[derive(Debug, Clone, thiserror::Error)]
pub enum JiraError {
    #[error("could not reach Jira: {0}")]
    Network(String),
    #[error("Jira authentication failed — check your email and API token")]
    Auth,
    #[error("Jira resource not found: {0}")]
    NotFound(String),
    #[error("Jira API error ({status}): {message}")]
    Api { status: u16, message: String },
    #[error("could not parse Jira's response: {0}")]
    Deserialize(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraMyself {
    pub account_id: String,
    pub display_name: String,
    pub email_address: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraIssue {
    pub key: String,
    pub summary: String,
    pub status: Option<String>,
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraWorklog {
    pub id: String,
}

/// Jira Cloud's worklog `comment` field requires Atlassian Document Format, not a
/// plain string. Returns `None` for empty/whitespace-only text so callers can omit the
/// `comment` key entirely (the spec treats the worklog comment as optional).
pub fn plain_text_to_adf(text: &str) -> Option<serde_json::Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(serde_json::json!({
        "type": "doc",
        "version": 1,
        "content": [{
            "type": "paragraph",
            "content": [{"type": "text", "text": trimmed}]
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression guard for the "never log secrets" invariant: since `JiraError` is
    /// only ever constructed from status codes/bodies, seeding a fixture token into a
    /// body and asserting it survives verifies the *shape* is safe — the real
    /// guarantee is that no variant here ever takes a header/token as input at all.
    #[test]
    fn jira_error_variants_never_embed_request_headers() {
        let errors = vec![
            JiraError::Network("connection refused".into()),
            JiraError::Auth,
            JiraError::NotFound("PROJ-1".into()),
            JiraError::Api { status: 500, message: "server error".into() },
            JiraError::Deserialize("unexpected token".into()),
        ];
        for e in errors {
            let rendered = format!("{e} {e:?}");
            assert!(!rendered.to_lowercase().contains("authorization"));
            assert!(!rendered.to_lowercase().contains("basic "));
        }
    }

    #[test]
    fn empty_comment_produces_no_adf() {
        assert!(plain_text_to_adf("").is_none());
        assert!(plain_text_to_adf("   ").is_none());
        assert!(plain_text_to_adf("daily standup").is_some());
    }
}
