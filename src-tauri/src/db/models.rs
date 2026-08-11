use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: i64,
    pub jira_key: String,
    pub summary: String,
    pub is_favorite: bool,
    pub is_assigned_to_me: bool,
    pub last_synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntry {
    pub id: i64,
    pub task_id: i64,
    pub task_key: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i64>,
    pub comment: Option<String>,
    pub is_synced: bool,
    pub jira_worklog_id: Option<String>,
    pub created_manually: bool,
    pub edited_at: Option<DateTime<Utc>>,
}

impl TimeEntry {
    pub fn is_running(&self) -> bool {
        self.ended_at.is_none()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsRow {
    pub jira_base_url: Option<String>,
    pub jira_email: Option<String>,
}
