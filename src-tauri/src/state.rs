use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{AppError, AppResult};
use crate::jira::JiraClient;

pub struct AppState {
    pub db: Mutex<Connection>,
    jira_client: Mutex<Option<Arc<dyn JiraClient>>>,
    tray_available: AtomicBool,
}

impl AppState {
    pub fn new(db: Connection) -> Self {
        Self {
            db: Mutex::new(db),
            jira_client: Mutex::new(None),
            tray_available: AtomicBool::new(false),
        }
    }

    pub fn set_jira_client(&self, client: Arc<dyn JiraClient>) {
        *self.jira_client.lock().unwrap() = Some(client);
    }

    pub fn clear_jira_client(&self) {
        *self.jira_client.lock().unwrap() = None;
    }

    pub fn get_jira_client(&self) -> Option<Arc<dyn JiraClient>> {
        self.jira_client.lock().unwrap().clone()
    }

    pub fn require_jira_client(&self) -> AppResult<Arc<dyn JiraClient>> {
        self.get_jira_client()
            .ok_or_else(|| AppError::Validation("Jira is not configured yet — finish Setup first.".into()))
    }

    pub fn set_tray_available(&self, available: bool) {
        self.tray_available.store(available, Ordering::SeqCst);
    }

    pub fn is_tray_available(&self) -> bool {
        self.tray_available.load(Ordering::SeqCst)
    }
}
