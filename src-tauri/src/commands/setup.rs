use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::db::settings_repo;
use crate::error::{AppError, AppResult};
use crate::jira::models::JiraMyself;
use crate::jira::reqwest_client::ReqwestJiraClient;
use crate::jira::JiraClient;
use crate::secrets::keyring_store;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub jira_base_url: Option<String>,
    pub jira_email: Option<String>,
    /// Never the token itself — just whether one is present in the OS keychain.
    pub has_token: bool,
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppResult<SettingsDto> {
    let settings = {
        let conn = state.db.lock().unwrap();
        settings_repo::get_settings(&conn)?
    };
    let has_token = keyring_store::load_token()?.is_some();
    Ok(SettingsDto {
        jira_base_url: settings.as_ref().and_then(|s| s.jira_base_url.clone()),
        jira_email: settings.as_ref().and_then(|s| s.jira_email.clone()),
        has_token,
    })
}

/// Validates the given credentials against a real `GET /myself` call *before* writing
/// anything. Only on success are the base URL/email persisted to SQLite and the token
/// written to the OS keychain; if the keychain write fails, the settings write is
/// rolled back so we never end up with a settings row and no token (or vice versa).
#[tauri::command]
pub async fn save_jira_settings(
    state: State<'_, AppState>,
    base_url: String,
    email: String,
    api_token: String,
) -> AppResult<JiraMyself> {
    let base_url = base_url.trim().trim_end_matches('/').to_string();
    let email = email.trim().to_string();
    let api_token = api_token.trim().to_string();
    if base_url.is_empty() || email.is_empty() || api_token.is_empty() {
        return Err(AppError::Validation(
            "Instance URL, email, and API token are all required.".into(),
        ));
    }

    let candidate = ReqwestJiraClient::new(base_url.clone(), email.clone(), api_token.clone());
    let myself = candidate.get_myself().await?;

    keyring_store::store_token(&api_token)?;

    let save_result = {
        let conn = state.db.lock().unwrap();
        settings_repo::save_settings(&conn, &base_url, &email)
    };
    if let Err(e) = save_result {
        let _ = keyring_store::delete_token();
        return Err(e.into());
    }

    state.set_jira_client(Arc::new(candidate));
    Ok(myself)
}

#[tauri::command]
pub async fn test_jira_connection(state: State<'_, AppState>) -> AppResult<JiraMyself> {
    let client = state.require_jira_client()?;
    Ok(client.get_myself().await?)
}

#[tauri::command]
pub fn clear_jira_settings(state: State<'_, AppState>) -> AppResult<()> {
    {
        let conn = state.db.lock().unwrap();
        settings_repo::clear_settings(&conn)?;
    }
    keyring_store::delete_token()?;
    state.clear_jira_client();
    Ok(())
}
