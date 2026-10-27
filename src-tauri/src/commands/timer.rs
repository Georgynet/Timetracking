use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::db::models::TimeEntry;
use crate::db::time_entries_repo;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::timer::engine;
use crate::tray;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveTimerDto {
    #[serde(flatten)]
    pub entry: TimeEntry,
    /// True once the timer has run longer than `engine::STALE_THRESHOLD_SECS` — the
    /// frontend uses this to show a non-blocking "still running?" banner instead of
    /// treating every resumed session as a crash.
    pub is_stale: bool,
}

#[tauri::command]
pub fn get_active_timer(state: State<'_, AppState>) -> AppResult<Option<ActiveTimerDto>> {
    let conn = state.db.lock().unwrap();
    let running = engine::get_running(&conn)?;
    Ok(running.map(|entry| {
        let is_stale = engine::is_stale(&entry, Utc::now());
        ActiveTimerDto { entry, is_stale }
    }))
}

#[tauri::command]
pub fn start_timer(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: i64,
    comment: Option<String>,
) -> AppResult<TimeEntry> {
    let entry = {
        let mut conn = state.db.lock().unwrap();
        engine::start(&mut conn, task_id, comment, Utc::now())
    };
    tray::refresh(&app);
    Ok(entry?)
}

/// `id` pins the note to the entry it was typed for: if that timer was stopped or
/// switched in the meantime (e.g. from the tray), a late save is rejected instead of
/// landing on whatever entry is running now.
fn update_running_comment_impl(
    state: &AppState,
    id: i64,
    comment: Option<String>,
) -> AppResult<TimeEntry> {
    let conn = state.db.lock().unwrap();
    match engine::get_running(&conn)? {
        Some(running) if running.id == id => {}
        _ => return Err(AppError::Validation("This timer is no longer running.".into())),
    }
    let comment = comment.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    Ok(time_entries_repo::set_running_comment(&conn, id, comment.as_deref())?)
}

/// Saves the note typed next to the running timer. It is stored on the running entry
/// itself, so stopping the timer from anywhere (UI, tray, switching tickets) keeps it.
#[tauri::command]
pub fn update_running_comment(
    state: State<'_, AppState>,
    id: i64,
    comment: Option<String>,
) -> AppResult<TimeEntry> {
    update_running_comment_impl(&state, id, comment)
}

#[tauri::command]
pub fn stop_timer(app: AppHandle, state: State<'_, AppState>) -> AppResult<TimeEntry> {
    let entry = {
        let conn = state.db.lock().unwrap();
        engine::stop(&conn, Utc::now())
    };
    tray::refresh(&app);
    Ok(entry?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;
    use crate::db::tasks_repo;
    use chrono::{DateTime, Duration, TimeZone};

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 10, 12, 0, 0).unwrap()
    }

    fn setup() -> (AppState, i64) {
        let conn = open_in_memory().unwrap();
        let task_id = tasks_repo::upsert_favorite_task(&conn, "PROJ-1", "Ticket", now()).unwrap().id;
        (AppState::new(conn), task_id)
    }

    #[test]
    fn note_on_the_running_timer_is_kept_when_it_stops() {
        let (state, task_id) = setup();
        let running = {
            let mut conn = state.db.lock().unwrap();
            engine::start(&mut conn, task_id, Some("from start".into()), now()).unwrap()
        };

        let updated =
            update_running_comment_impl(&state, running.id, Some("  reviewed the PR  ".into()))
                .unwrap();
        assert_eq!(updated.comment.as_deref(), Some("reviewed the PR"));
        assert!(updated.edited_at.is_none(), "a live note is not an after-the-fact edit");

        let conn = state.db.lock().unwrap();
        let stopped = engine::stop(&conn, now() + Duration::minutes(5)).unwrap();
        assert_eq!(stopped.comment.as_deref(), Some("reviewed the PR"));
    }

    #[test]
    fn clearing_the_note_stores_no_comment() {
        let (state, task_id) = setup();
        let running = {
            let mut conn = state.db.lock().unwrap();
            engine::start(&mut conn, task_id, Some("draft".into()), now()).unwrap()
        };
        let updated = update_running_comment_impl(&state, running.id, Some("   ".into())).unwrap();
        assert_eq!(updated.comment, None);
    }

    #[test]
    fn late_note_for_a_stopped_timer_is_rejected() {
        let (state, task_id) = setup();
        let stopped = {
            let mut conn = state.db.lock().unwrap();
            let first = engine::start(&mut conn, task_id, None, now()).unwrap();
            // Switching tickets stops `first` and starts a new running entry.
            engine::start(&mut conn, task_id, None, now() + Duration::minutes(1)).unwrap();
            first
        };

        let result = update_running_comment_impl(&state, stopped.id, Some("late".into()));
        assert!(result.is_err());
        let conn = state.db.lock().unwrap();
        assert_eq!(engine::get_running(&conn).unwrap().unwrap().comment, None);
        assert_eq!(time_entries_repo::get_by_id(&conn, stopped.id).unwrap().unwrap().comment, None);
    }
}
