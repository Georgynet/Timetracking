use serde::Serialize;
use tauri::{AppHandle, State};

use crate::db::time_entries_repo;
use crate::error::AppResult;
use crate::state::AppState;
use crate::sync::service;
use crate::tray;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFailure {
    pub entry_id: i64,
    pub task_key: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReportDto {
    pub total: usize,
    pub succeeded: Vec<i64>,
    pub failed: Vec<SyncFailure>,
}

/// Shared by the `sync_all` command and the tray's "Sync to Jira" item.
pub async fn sync_all_impl(state: &AppState) -> AppResult<SyncReportDto> {
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
pub async fn sync_all(app: AppHandle, state: State<'_, AppState>) -> AppResult<SyncReportDto> {
    let report = sync_all_impl(&state).await;
    tray::update_texts(&app);
    report
}

#[tauri::command]
pub fn list_unsynced_count(state: State<'_, AppState>) -> AppResult<i64> {
    let conn = state.db.lock().unwrap();
    Ok(time_entries_repo::count_unsynced(&conn)?)
}
