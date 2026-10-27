//! System notifications (macOS only, ADR-0038): results of actions taken while the
//! window is usually hidden, and background reminders checked every 30 seconds.
//! Desktop notifications can't carry action buttons in `tauri-plugin-notification`, so
//! they only inform — acting on them goes through the tray or the window.

pub mod reminders;
pub mod settings;

use std::time::Duration;

use chrono::Utc;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::commands::sync::SyncReportDto;
use crate::db::time_entries_repo;
use crate::state::AppState;

const CHECK_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub title: String,
    pub body: String,
}

impl Notice {
    pub fn new(title: &str, body: String) -> Self {
        Self {
            title: title.into(),
            body,
        }
    }
}

/// Linux notification daemons vary and can't be verified here, so notifications are a
/// macOS feature, like the other menu bar extras (ADR-0037).
pub fn supported() -> bool {
    cfg!(target_os = "macos")
}

fn show(app: &AppHandle, notice: &Notice) {
    let result = app
        .notification()
        .builder()
        .title(&notice.title)
        .body(&notice.body)
        .show();
    if let Err(err) = result {
        tracing::warn!("failed to show notification: {err}");
    }
}

/// Current notification settings, or `None` (and a log line) if unsupported or the
/// read fails. Callers must not hold the DB lock.
fn load_settings(app: &AppHandle) -> Option<settings::NotificationSettings> {
    if !supported() {
        return None;
    }
    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap();
    settings::load(&conn)
        .map_err(|err| tracing::warn!("failed to read notification settings: {err}"))
        .ok()
}

/// Starts the background reminder checks. A no-op where notifications aren't supported.
pub fn start(app: AppHandle) {
    if !supported() {
        return;
    }
    std::thread::spawn(move || {
        let mut memory = reminders::ReminderMemory::default();
        loop {
            std::thread::sleep(CHECK_INTERVAL);
            check_reminders(&app, &mut memory);
        }
    });
}

fn check_reminders(app: &AppHandle, memory: &mut reminders::ReminderMemory) {
    let Some(settings) = load_settings(app) else { return };
    let now = Utc::now();
    let inputs = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        reminders::load_inputs(&conn, now)
    };
    match inputs {
        Ok(inputs) => {
            for notice in reminders::due(&inputs, &settings, memory, now) {
                show(app, &notice);
            }
        }
        Err(err) => tracing::warn!("failed to read reminder state: {err}"),
    }
}

/// After a sync started from the tray, where no report modal is in view.
pub fn sync_finished(app: &AppHandle, report: &SyncReportDto) {
    if !load_settings(app).is_some_and(|s| s.sync_result) {
        return;
    }
    let ok = report.succeeded.len();
    let notice = match report.failed.len() {
        0 => Notice::new("Synced to Jira", format!("{ok} {} synced.", entries(ok))),
        failed => Notice::new(
            "Sync finished with errors",
            format!("{ok} synced, {failed} failed — open Time Tracker for details."),
        ),
    };
    show(app, &notice);
}

/// After the workday is ended (from the tray or the UI): a nudge if time is still
/// waiting to be pushed to Jira. Callers must not hold the DB lock.
pub fn workday_ended(app: &AppHandle) {
    if !load_settings(app).is_some_and(|s| s.unsynced_on_end_workday) {
        return;
    }
    let unsynced = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        time_entries_repo::count_unsynced(&conn)
    };
    match unsynced {
        Ok(0) => {}
        Ok(n) => show(
            app,
            &Notice::new(
                "Unsynced time",
                format!("{n} {} not synced to Jira yet.", entries(n as usize)),
            ),
        ),
        Err(err) => tracing::warn!("failed to count unsynced entries: {err}"),
    }
}

fn entries(n: usize) -> &'static str {
    if n == 1 {
        "entry"
    } else {
        "entries"
    }
}
