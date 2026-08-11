use chrono::Utc;
use tauri::State;

use crate::db::models::Task;
use crate::db::tasks_repo;
use crate::error::AppResult;
use crate::jira::models::JiraIssue;
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

/// Refetches "my" tickets via JQL and re-populates `is_assigned_to_me`. Never touches
/// `is_favorite` — a ticket can be assigned-to-me, a favorite, both, or neither, and
/// the two lists are managed independently.
#[tauri::command]
pub async fn refresh_my_tasks(state: State<'_, AppState>) -> AppResult<Vec<Task>> {
    let client = state.require_jira_client()?;
    let jql = "assignee = currentUser() AND resolution = Unresolved";
    let issues = client.search_issues(jql, 100).await?;

    let now = Utc::now();
    let conn = state.db.lock().unwrap();
    tasks_repo::reset_assigned_to_me(&conn)?;
    for issue in &issues {
        tasks_repo::upsert_assigned_task(&conn, &issue.key, &issue.summary, now)?;
    }
    Ok(tasks_repo::list_my_tasks(&conn)?)
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
}
