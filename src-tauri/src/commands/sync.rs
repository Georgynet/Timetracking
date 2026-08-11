use serde::Serialize;
use tauri::State;

use crate::db::time_entries_repo;
use crate::error::AppResult;
use crate::state::AppState;
use crate::sync::service;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFailure {
    pub entry_id: i64,
    pub task_key: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReportDto {
    pub total: usize,
    pub succeeded: Vec<i64>,
    pub failed: Vec<SyncFailure>,
}

#[tauri::command]
pub async fn sync_all(state: State<'_, AppState>) -> AppResult<SyncReportDto> {
    let client = state.require_jira_client()?;
    let outcomes = service::sync_all(&state.db, &*client).await;

    let mut report = SyncReportDto {
        total: outcomes.len(),
        succeeded: Vec::new(),
        failed: Vec::new(),
    };
    for outcome in outcomes {
        match outcome.result {
            Ok(_) => report.succeeded.push(outcome.entry_id),
            Err(message) => report.failed.push(SyncFailure {
                entry_id: outcome.entry_id,
                task_key: outcome.task_key,
                message,
            }),
        }
    }
    Ok(report)
}

#[tauri::command]
pub fn list_unsynced_count(state: State<'_, AppState>) -> AppResult<i64> {
    let conn = state.db.lock().unwrap();
    Ok(time_entries_repo::count_unsynced(&conn)?)
}
