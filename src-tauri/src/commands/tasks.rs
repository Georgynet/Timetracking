use std::collections::HashSet;

use chrono::Utc;
use tauri::State;

use crate::db::models::Task;
use crate::db::tasks_repo;
use crate::error::AppResult;
use crate::jira::models::JiraIssue;
use crate::jira::JiraClient;
use crate::state::AppState;

/// Matches a Jira issue key like `PROJ-123`: an all-caps (letters+digits) project
/// prefix starting with a letter, a dash, then digits. Written by hand instead of
/// pulling in the `regex` crate for one small check.
fn looks_like_issue_key(s: &str) -> bool {
    let Some((project, number)) = s.rsplit_once('-') else {
        return false;
    };
    let mut chars = project.chars();
    let Some(first) = chars.next() else { return false };
    if !first.is_ascii_uppercase() {
        return false;
    }
    if !chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return false;
    }
    !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
}

/// Which of `assignee = currentUser() AND resolution = Unresolved`'s results are also
/// in a currently active sprint, keyed by issue key. Best-effort: `sprint in
/// openSprints()` errors out on Jira sites/projects with no Scrum board at all (the
/// `sprint` field doesn't exist for them), so a failure here just means "nothing is in
/// the current sprint" rather than failing the whole "my tasks" refresh.
async fn current_sprint_keys(client: &dyn JiraClient) -> HashSet<String> {
    let jql = "assignee = currentUser() AND resolution = Unresolved AND sprint in openSprints()";
    match client.search_issues(jql, 100).await {
        Ok(issues) => issues.into_iter().map(|i| i.key).collect(),
        Err(err) => {
            tracing::warn!("sprint lookup failed, treating as no active sprint: {err}");
            HashSet::new()
        }
    }
}

/// Refetches "my" tickets via JQL and re-populates `is_assigned_to_me` and
/// `is_in_current_sprint`. Never touches `is_favorite` — a ticket can be
/// assigned-to-me, a favorite, both, or neither, and the two lists are managed
/// independently.
async fn refresh_my_tasks_impl(state: &AppState) -> AppResult<Vec<Task>> {
    let client = state.require_jira_client()?;
    let jql = "assignee = currentUser() AND resolution = Unresolved";
    let issues = client.search_issues(jql, 100).await?;
    let sprint_keys = current_sprint_keys(client.as_ref()).await;

    let now = Utc::now();
    let conn = state.db.lock().unwrap();
    tasks_repo::reset_assigned_to_me(&conn)?;
    for issue in &issues {
        let in_current_sprint = sprint_keys.contains(&issue.key);
        tasks_repo::upsert_assigned_task(&conn, &issue.key, &issue.summary, in_current_sprint, now)?;
    }
    Ok(tasks_repo::list_my_tasks(&conn)?)
}

#[tauri::command]
pub async fn refresh_my_tasks(state: State<'_, AppState>) -> AppResult<Vec<Task>> {
    refresh_my_tasks_impl(&state).await
}

#[tauri::command]
pub fn list_my_tasks(state: State<'_, AppState>) -> AppResult<Vec<Task>> {
    let conn = state.db.lock().unwrap();
    Ok(tasks_repo::list_my_tasks(&conn)?)
}

#[tauri::command]
pub fn list_favorite_tasks(state: State<'_, AppState>) -> AppResult<Vec<Task>> {
    let conn = state.db.lock().unwrap();
    Ok(tasks_repo::list_favorite_tasks(&conn)?)
}

/// Preview-only lookup (no DB write) used by the "add favorite" search box: a direct
/// issue key gets an exact lookup, anything else is treated as free-text/JQL.
#[tauri::command]
pub async fn search_jira_issues(state: State<'_, AppState>, query: String) -> AppResult<Vec<JiraIssue>> {
    let client = state.require_jira_client()?;
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    if looks_like_issue_key(&query.to_uppercase()) {
        let issue = client.get_issue(&query.to_uppercase()).await?;
        return Ok(vec![issue]);
    }
    let jql = format!("text ~ \"{}\" ORDER BY updated DESC", query.replace('"', "\\\""));
    Ok(client.search_issues(&jql, 25).await?)
}

#[tauri::command]
pub async fn add_favorite_by_key(state: State<'_, AppState>, jira_key: String) -> AppResult<Task> {
    let client = state.require_jira_client()?;
    let jira_key = jira_key.trim().to_uppercase();
    let issue = client.get_issue(&jira_key).await?;

    let conn = state.db.lock().unwrap();
    Ok(tasks_repo::upsert_favorite_task(&conn, &issue.key, &issue.summary, Utc::now())?)
}

#[tauri::command]
pub fn remove_favorite(state: State<'_, AppState>, task_id: i64) -> AppResult<()> {
    let conn = state.db.lock().unwrap();
    tasks_repo::remove_favorite(&conn, task_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;
    use crate::jira::fake_client::FakeJiraClient;
    use crate::jira::models::{JiraError, JiraIssue};
    use std::sync::Arc;

    #[test]
    fn recognizes_issue_keys() {
        assert!(looks_like_issue_key("PROJ-123"));
        assert!(looks_like_issue_key("TEAM-1"));
        assert!(looks_like_issue_key("A1-42"));
        assert!(!looks_like_issue_key("daily standup"));
        assert!(!looks_like_issue_key("proj-123"));
        assert!(!looks_like_issue_key("PROJ"));
        assert!(!looks_like_issue_key("-123"));
        assert!(!looks_like_issue_key("PROJ-"));
    }

    fn issue(key: &str) -> JiraIssue {
        JiraIssue { key: key.into(), summary: format!("Summary for {key}"), status: None, project: None }
    }

    fn setup(client: FakeJiraClient) -> AppState {
        let state = AppState::new(open_in_memory().unwrap());
        state.set_jira_client(Arc::new(client));
        state
    }

    #[tokio::test]
    async fn refresh_marks_only_issues_in_the_current_sprint() {
        let mut client = FakeJiraClient::default();
        client.search_results = vec![issue("PROJ-1"), issue("PROJ-2")];
        client.sprint_search_results = Ok(vec![issue("PROJ-1")]);
        let state = setup(client);

        let tasks = refresh_my_tasks_impl(&state).await.unwrap();

        let proj1 = tasks.iter().find(|t| t.jira_key == "PROJ-1").unwrap();
        let proj2 = tasks.iter().find(|t| t.jira_key == "PROJ-2").unwrap();
        assert!(proj1.is_in_current_sprint);
        assert!(!proj2.is_in_current_sprint);
    }

    #[tokio::test]
    async fn a_failing_sprint_lookup_does_not_fail_the_whole_refresh() {
        let mut client = FakeJiraClient::default();
        client.search_results = vec![issue("PROJ-1")];
        client.sprint_search_results = Err(JiraError::Api { status: 400, message: "field 'sprint' does not exist".into() });
        let state = setup(client);

        let tasks = refresh_my_tasks_impl(&state).await.unwrap();

        assert_eq!(tasks.len(), 1);
        assert!(!tasks[0].is_in_current_sprint, "no active sprint data means nothing is flagged as in-sprint");
    }

    #[tokio::test]
    async fn re_refreshing_without_a_task_still_in_the_sprint_clears_its_flag() {
        let mut client = FakeJiraClient::default();
        client.search_results = vec![issue("PROJ-1")];
        client.sprint_search_results = Ok(vec![issue("PROJ-1")]);
        let state = setup(client);
        refresh_my_tasks_impl(&state).await.unwrap();

        // Second refresh: PROJ-1 is still assigned, but the sprint moved on.
        let mut client = FakeJiraClient::default();
        client.search_results = vec![issue("PROJ-1")];
        client.sprint_search_results = Ok(vec![]);
        state.set_jira_client(Arc::new(client));

        let tasks = refresh_my_tasks_impl(&state).await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert!(!tasks[0].is_in_current_sprint);
    }
}
