use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::client_trait::JiraClient;
use super::models::{plain_text_to_adf, JiraError, JiraIssue, JiraMyself, JiraWorklog};

pub struct ReqwestJiraClient {
    http: reqwest::Client,
    base_url: String,
    email: String,
    api_token: String,
}

impl ReqwestJiraClient {
    pub fn new(base_url: impl Into<String>, email: impl Into<String>, api_token: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            email: email.into(),
            api_token: api_token.into(),
        }
    }

    fn auth_header(&self) -> String {
        format!("Basic {}", STANDARD.encode(format!("{}:{}", self.email, self.api_token)))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Turns a `reqwest::Response` into `Ok(T)` or a `JiraError` built only from the
    /// status code and response body — never from this client's own request headers.
    async fn handle_response<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, JiraError> {
        let status = resp.status();
        if status.is_success() {
            let bytes = resp.bytes().await.map_err(|e| JiraError::Network(e.to_string()))?;
            serde_json::from_slice(&bytes).map_err(|e| JiraError::Deserialize(e.to_string()))
        } else {
            let body = resp.text().await.unwrap_or_default();
            match status.as_u16() {
                401 | 403 => Err(JiraError::Auth),
                404 => Err(JiraError::NotFound(body)),
                other => Err(JiraError::Api { status: other, message: truncate(&body, 500) }),
            }
        }
    }

    fn started_format(dt: DateTime<Utc>) -> String {
        dt.format("%Y-%m-%dT%H:%M:%S%.3f%z").to_string()
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[derive(Deserialize)]
struct MyselfResponse {
    #[serde(rename = "accountId")]
    account_id: String,
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "emailAddress")]
    email_address: String,
}

#[derive(Deserialize)]
struct IssueFields {
    summary: String,
    status: Option<IssueStatus>,
    project: Option<IssueProject>,
}

#[derive(Deserialize)]
struct IssueStatus {
    name: String,
}

#[derive(Deserialize)]
struct IssueProject {
    key: String,
}

#[derive(Deserialize)]
struct IssueResponse {
    key: String,
    fields: IssueFields,
}

impl From<IssueResponse> for JiraIssue {
    fn from(r: IssueResponse) -> Self {
        JiraIssue {
            key: r.key,
            summary: r.fields.summary,
            status: r.fields.status.map(|s| s.name),
            project: r.fields.project.map(|p| p.key),
        }
    }
}

#[derive(Deserialize)]
struct SearchResponse {
    issues: Vec<IssueResponse>,
}

#[derive(Serialize)]
struct SearchRequest<'a> {
    jql: &'a str,
    #[serde(rename = "maxResults")]
    max_results: u32,
    fields: &'static [&'static str],
}

#[derive(Serialize)]
struct WorklogRequest {
    started: String,
    #[serde(rename = "timeSpentSeconds")]
    time_spent_seconds: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    comment: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct WorklogResponse {
    id: String,
}

impl From<WorklogResponse> for JiraWorklog {
    fn from(r: WorklogResponse) -> Self {
        JiraWorklog { id: r.id }
    }
}

#[async_trait]
impl JiraClient for ReqwestJiraClient {
    async fn get_myself(&self) -> Result<JiraMyself, JiraError> {
        let resp = self
            .http
            .get(self.url("/rest/api/3/myself"))
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| JiraError::Network(e.to_string()))?;
        let parsed: MyselfResponse = Self::handle_response(resp).await?;
        Ok(JiraMyself {
            account_id: parsed.account_id,
            display_name: parsed.display_name,
            email_address: parsed.email_address,
        })
    }

    async fn search_issues(&self, jql: &str, max_results: u32) -> Result<Vec<JiraIssue>, JiraError> {
        let resp = self
            .http
            .post(self.url("/rest/api/3/search/jql"))
            .header("Authorization", self.auth_header())
            .json(&SearchRequest { jql, max_results, fields: &["summary", "status", "project"] })
            .send()
            .await
            .map_err(|e| JiraError::Network(e.to_string()))?;
        let parsed: SearchResponse = Self::handle_response(resp).await?;
        Ok(parsed.issues.into_iter().map(JiraIssue::from).collect())
    }

    async fn get_issue(&self, key_or_id: &str) -> Result<JiraIssue, JiraError> {
        let resp = self
            .http
            .get(self.url(&format!("/rest/api/3/issue/{key_or_id}?fields=summary,status,project")))
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| JiraError::Network(e.to_string()))?;
        let parsed: IssueResponse = Self::handle_response(resp).await?;
        Ok(JiraIssue::from(parsed))
    }

    async fn add_worklog(
        &self,
        issue_key: &str,
        started: DateTime<Utc>,
        seconds: i64,
        comment: Option<&str>,
    ) -> Result<JiraWorklog, JiraError> {
        let body = WorklogRequest {
            started: Self::started_format(started),
            time_spent_seconds: seconds,
            comment: comment.and_then(plain_text_to_adf),
        };
        let resp = self
            .http
            .post(self.url(&format!("/rest/api/3/issue/{issue_key}/worklog")))
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| JiraError::Network(e.to_string()))?;
        let parsed: WorklogResponse = Self::handle_response(resp).await?;
        Ok(JiraWorklog::from(parsed))
    }

    async fn update_worklog(
        &self,
        issue_key: &str,
        worklog_id: &str,
        started: DateTime<Utc>,
        seconds: i64,
        comment: Option<&str>,
    ) -> Result<JiraWorklog, JiraError> {
        let body = WorklogRequest {
            started: Self::started_format(started),
            time_spent_seconds: seconds,
            comment: comment.and_then(plain_text_to_adf),
        };
        let resp = self
            .http
            .put(self.url(&format!("/rest/api/3/issue/{issue_key}/worklog/{worklog_id}")))
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| JiraError::Network(e.to_string()))?;
        let parsed: WorklogResponse = Self::handle_response(resp).await?;
        Ok(JiraWorklog::from(parsed))
    }
}
