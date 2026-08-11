use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::models::{JiraError, JiraIssue, JiraMyself, JiraWorklog};

/// Abstraction over the Jira Cloud REST API v3 calls this app needs. Exists so
/// business logic (timer engine, sync service) can be tested against
/// `jira::fake_client::FakeJiraClient` without any HTTP involved, while
/// `jira::reqwest_client::ReqwestJiraClient` is the real implementation used at
/// runtime.
#[async_trait]
pub trait JiraClient: Send + Sync {
    async fn get_myself(&self) -> Result<JiraMyself, JiraError>;

    async fn search_issues(&self, jql: &str, max_results: u32) -> Result<Vec<JiraIssue>, JiraError>;

    async fn get_issue(&self, key_or_id: &str) -> Result<JiraIssue, JiraError>;

    async fn add_worklog(
        &self,
        issue_key: &str,
        started: DateTime<Utc>,
        seconds: i64,
        comment: Option<&str>,
    ) -> Result<JiraWorklog, JiraError>;

    async fn update_worklog(
        &self,
        issue_key: &str,
        worklog_id: &str,
        started: DateTime<Utc>,
        seconds: i64,
        comment: Option<&str>,
    ) -> Result<JiraWorklog, JiraError>;
}
