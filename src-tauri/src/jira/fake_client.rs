use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::client_trait::JiraClient;
use super::models::{JiraError, JiraIssue, JiraMyself, JiraWorklog};

#[derive(Debug, Clone)]
pub struct RecordedWorklogCall {
    pub issue_key: String,
    /// `Some` for an update call (PUT), `None` for a create call (POST).
    pub worklog_id: Option<String>,
    pub seconds: i64,
}

/// Trait-based fake `JiraClient` for fast, HTTP-free tests of business logic (timer
/// engine, sync service) — no network involved, deterministic, and can simulate
/// per-issue failures (e.g. "one entry fails, the rest still sync").
pub struct FakeJiraClient {
    pub myself: Result<JiraMyself, JiraError>,
    pub issues_by_key: HashMap<String, JiraIssue>,
    pub search_results: Vec<JiraIssue>,
    /// What a sprint-scoped search (any JQL containing `openSprints()` — see
    /// `commands::tasks::refresh_my_tasks`) returns. `Ok(vec![])` by default, distinct
    /// from `search_results`, so tests can simulate "some of my tasks are in the
    /// active sprint" without the fake needing to actually parse JQL.
    pub sprint_search_results: Result<Vec<JiraIssue>, JiraError>,
    /// Issue keys present here return this canned outcome for add/update worklog;
    /// keys absent default to a successful fake worklog id.
    pub worklog_outcomes: HashMap<String, Result<JiraWorklog, JiraError>>,
    pub calls: Mutex<Vec<RecordedWorklogCall>>,
    next_id: AtomicI64,
}

impl Default for FakeJiraClient {
    fn default() -> Self {
        Self {
            myself: Ok(JiraMyself {
                account_id: "fake-account".into(),
                display_name: "Fake User".into(),
                email_address: "fake@example.com".into(),
            }),
            issues_by_key: HashMap::new(),
            search_results: Vec::new(),
            sprint_search_results: Ok(Vec::new()),
            worklog_outcomes: HashMap::new(),
            calls: Mutex::new(Vec::new()),
            next_id: AtomicI64::new(1),
        }
    }
}

impl FakeJiraClient {
    fn outcome_for(&self, issue_key: &str) -> Result<JiraWorklog, JiraError> {
        match self.worklog_outcomes.get(issue_key) {
            Some(outcome) => outcome.clone(),
            None => Ok(JiraWorklog {
                id: format!("fake-{}", self.next_id.fetch_add(1, Ordering::SeqCst)),
            }),
        }
    }
}

#[async_trait]
impl JiraClient for FakeJiraClient {
    async fn get_myself(&self) -> Result<JiraMyself, JiraError> {
        self.myself.clone()
    }

    async fn search_issues(&self, jql: &str, _max_results: u32) -> Result<Vec<JiraIssue>, JiraError> {
        if jql.contains("openSprints()") {
            self.sprint_search_results.clone()
        } else {
            Ok(self.search_results.clone())
        }
    }

    async fn get_issue(&self, key_or_id: &str) -> Result<JiraIssue, JiraError> {
        self.issues_by_key
            .get(key_or_id)
            .cloned()
            .ok_or_else(|| JiraError::NotFound(key_or_id.to_string()))
    }

    async fn add_worklog(
        &self,
        issue_key: &str,
        _started: DateTime<Utc>,
        seconds: i64,
        _comment: Option<&str>,
    ) -> Result<JiraWorklog, JiraError> {
        self.calls.lock().unwrap().push(RecordedWorklogCall {
            issue_key: issue_key.to_string(),
            worklog_id: None,
            seconds,
        });
        self.outcome_for(issue_key)
    }

    async fn update_worklog(
        &self,
        issue_key: &str,
        worklog_id: &str,
        _started: DateTime<Utc>,
        seconds: i64,
        _comment: Option<&str>,
    ) -> Result<JiraWorklog, JiraError> {
        self.calls.lock().unwrap().push(RecordedWorklogCall {
            issue_key: issue_key.to_string(),
            worklog_id: Some(worklog_id.to_string()),
            seconds,
        });
        self.outcome_for(issue_key)
    }
}
