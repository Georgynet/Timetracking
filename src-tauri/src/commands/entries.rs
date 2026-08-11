use chrono::{DateTime, Duration, Utc};
use tauri::State;

use crate::db::models::TimeEntry;
use crate::db::time_entries_repo::{self, EntryUpdate};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

fn parse_dt(s: &str) -> AppResult<DateTime<Utc>> {
    s.parse::<DateTime<Utc>>()
        .map_err(|_| AppError::Validation(format!("Invalid date/time: {s}")))
}

/// Resolves the (end, duration) pair for a brand-new manual entry: an explicit end
/// time wins if given, otherwise it's derived from the duration. At least one of the
/// two must be supplied.
fn resolve_new_bounds(
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    duration_seconds: Option<i64>,
) -> AppResult<(DateTime<Utc>, i64)> {
    match (ended_at, duration_seconds) {
        (Some(end), _) => {
            let seconds = (end - started_at).num_seconds();
            if seconds < 0 {
                return Err(AppError::Validation("End time must be after start time.".into()));
            }
            Ok((end, seconds))
        }
        (None, Some(seconds)) => {
            if seconds < 0 {
                return Err(AppError::Validation("Duration must be positive.".into()));
            }
            Ok((started_at + Duration::seconds(seconds), seconds))
        }
        (None, None) => Err(AppError::Validation(
            "Provide either an end time or a duration.".into(),
        )),
    }
}

fn create_manual_entry_impl(
    state: &AppState,
    task_id: i64,
    started_at: String,
    ended_at: Option<String>,
    duration_seconds: Option<i64>,
    comment: Option<String>,
) -> AppResult<TimeEntry> {
    let started_at = parse_dt(&started_at)?;
    let ended_at = ended_at.map(|s| parse_dt(&s)).transpose()?;
    let (ended_at, duration) = resolve_new_bounds(started_at, ended_at, duration_seconds)?;

    let conn = state.db.lock().unwrap();
    Ok(time_entries_repo::insert_manual(
        &conn,
        task_id,
        started_at,
        ended_at,
        duration,
        comment.as_deref(),
    )?)
}

#[tauri::command]
pub fn create_manual_entry(
    state: State<'_, AppState>,
    task_id: i64,
    started_at: String,
    ended_at: Option<String>,
    duration_seconds: Option<i64>,
    comment: Option<String>,
) -> AppResult<TimeEntry> {
    create_manual_entry_impl(&state, task_id, started_at, ended_at, duration_seconds, comment)
}

/// Edits an existing entry. If `started_at` moves but neither `ended_at` nor
/// `duration_seconds` is given, the existing length is preserved (the end time shifts
/// with the start). An explicit `ended_at` always wins over `duration_seconds` when
/// both are given.
///
/// Only a pending (`is_synced = false`), completed entry can be edited — the running
/// timer's lifecycle belongs exclusively to `timer::engine` (see ADR-0013), and once
/// an entry has been synced it's treated as a permanent record matching Jira: fixing
/// a mistake in a synced entry has to happen in Jira itself, not by editing the local
/// copy back out of sync with it.
///
/// `comment` is a plain `Option<String>`, not the "does this JSON key round-trip
/// `null` vs missing" double-option shape: `None` means "leave the comment
/// unchanged", `Some("")` clears it, `Some(text)` sets it. A nested
/// `Option<Option<String>>` command parameter can't be told apart from a bare JSON
/// `null` on the wire, so this flatter shape is what the frontend actually sends.
fn update_time_entry_impl(
    state: &AppState,
    id: i64,
    task_id: Option<i64>,
    started_at: Option<String>,
    ended_at: Option<String>,
    duration_seconds: Option<i64>,
    comment: Option<String>,
) -> AppResult<TimeEntry> {
    let conn = state.db.lock().unwrap();
    let current = time_entries_repo::get_by_id(&conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("No time entry with id {id}")))?;
    if current.is_running() {
        return Err(AppError::Validation(
            "Cannot edit the currently running timer — stop it first.".into(),
        ));
    }
    if current.is_synced {
        return Err(AppError::Validation(
            "Cannot edit an entry that has already been synced to Jira.".into(),
        ));
    }

    let new_started_at = started_at.map(|s| parse_dt(&s)).transpose()?;
    let new_ended_at = ended_at.map(|s| parse_dt(&s)).transpose()?;
    let effective_start = new_started_at.unwrap_or(current.started_at);

    let (resolved_end, resolved_duration) = if let Some(end) = new_ended_at {
        let seconds = (end - effective_start).num_seconds();
        if seconds < 0 {
            return Err(AppError::Validation("End time must be after start time.".into()));
        }
        (Some(end), Some(seconds))
    } else if let Some(seconds) = duration_seconds {
        if seconds < 0 {
            return Err(AppError::Validation("Duration must be positive.".into()));
        }
        (Some(effective_start + Duration::seconds(seconds)), Some(seconds))
    } else if new_started_at.is_some() {
        let existing_duration = current.duration_seconds.unwrap_or(0);
        (Some(effective_start + Duration::seconds(existing_duration)), Some(existing_duration))
    } else {
        (None, None)
    };

    let update = EntryUpdate {
        task_id,
        started_at: new_started_at,
        ended_at: resolved_end,
        duration_seconds: resolved_duration,
        comment: comment.map(|s| if s.is_empty() { None } else { Some(s) }),
    };
    Ok(time_entries_repo::update_entry(&conn, id, update, Utc::now())?)
}

#[tauri::command]
pub fn update_time_entry(
    state: State<'_, AppState>,
    id: i64,
    task_id: Option<i64>,
    started_at: Option<String>,
    ended_at: Option<String>,
    duration_seconds: Option<i64>,
    comment: Option<String>,
) -> AppResult<TimeEntry> {
    update_time_entry_impl(&state, id, task_id, started_at, ended_at, duration_seconds, comment)
}

#[tauri::command]
pub fn list_time_entries(
    state: State<'_, AppState>,
    task_id: Option<i64>,
    from: Option<String>,
    to: Option<String>,
) -> AppResult<Vec<TimeEntry>> {
    let from = from.map(|s| parse_dt(&s)).transpose()?;
    let to = to.map(|s| parse_dt(&s)).transpose()?;
    let conn = state.db.lock().unwrap();
    Ok(time_entries_repo::list_entries(&conn, task_id, from, to)?)
}

/// Hard-deletes an entry — any completed, never-synced entry, whether it was
/// created manually or via the timer. Once an entry has been synced (or ever carried
/// a `jira_worklog_id`, even if a since-blocked edit path could otherwise have reset
/// `is_synced` back to false — see ADR-0013/0007) it's permanent, matching the spec's
/// "never deleted" rule for anything that ever reached Jira. The running timer can
/// never be deleted through this path either — its lifecycle belongs to
/// `timer::engine` alone.
fn delete_draft_entry_impl(state: &AppState, id: i64) -> AppResult<()> {
    let conn = state.db.lock().unwrap();
    let entry = time_entries_repo::get_by_id(&conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("No time entry with id {id}")))?;
    if entry.is_running() {
        return Err(AppError::Validation(
            "Cannot delete the currently running timer — stop it first.".into(),
        ));
    }
    if entry.is_synced || entry.jira_worklog_id.is_some() {
        return Err(AppError::Validation(
            "Only entries that have never been synced to Jira can be deleted.".into(),
        ));
    }
    time_entries_repo::delete_draft(&conn, id)?;
    Ok(())
}

#[tauri::command]
pub fn delete_draft_entry(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    delete_draft_entry_impl(&state, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;
    use crate::db::tasks_repo;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 10, 12, 0, 0).unwrap()
    }

    fn setup() -> (AppState, i64) {
        let conn = open_in_memory().unwrap();
        let task_id = tasks_repo::upsert_favorite_task(&conn, "PROJ-1", "Ticket", now()).unwrap().id;
        (AppState::new(conn), task_id)
    }

    #[test]
    fn editing_the_running_timer_is_rejected() {
        let (state, task_id) = setup();
        let running = {
            let conn = state.db.lock().unwrap();
            time_entries_repo::insert_running(&conn, task_id, now(), None).unwrap()
        };

        let result = update_time_entry_impl(
            &state,
            running.id,
            None,
            None,
            Some((now() + Duration::hours(1)).to_rfc3339()),
            None,
            None,
        );

        assert!(result.is_err(), "editing the running timer must be rejected");
        let conn = state.db.lock().unwrap();
        let reloaded = time_entries_repo::get_by_id(&conn, running.id).unwrap().unwrap();
        assert!(reloaded.is_running(), "the entry must still be running, untouched, after the rejected edit");
    }

    #[test]
    fn shifting_start_time_alone_preserves_duration() {
        let (state, task_id) = setup();
        let entry =
            create_manual_entry_impl(&state, task_id, now().to_rfc3339(), None, Some(3600), None).unwrap();

        let new_start = now() + Duration::hours(1);
        let updated =
            update_time_entry_impl(&state, entry.id, None, Some(new_start.to_rfc3339()), None, None, None)
                .unwrap();

        assert_eq!(updated.duration_seconds, Some(3600), "duration must be preserved when only start moves");
        assert_eq!(updated.ended_at, Some(new_start + Duration::hours(1)));
    }

    #[test]
    fn explicit_end_time_overrides_duration_when_both_given() {
        let (state, task_id) = setup();
        let entry =
            create_manual_entry_impl(&state, task_id, now().to_rfc3339(), None, Some(3600), None).unwrap();

        let new_end = now() + Duration::minutes(15);
        let updated = update_time_entry_impl(
            &state,
            entry.id,
            None,
            None,
            Some(new_end.to_rfc3339()),
            Some(99999),
            None,
        )
        .unwrap();

        assert_eq!(updated.duration_seconds, Some(15 * 60));
    }

    #[test]
    fn editing_a_synced_entry_is_rejected() {
        let (state, task_id) = setup();
        let entry = create_manual_entry_impl(&state, task_id, now().to_rfc3339(), None, Some(60), None).unwrap();
        {
            let conn = state.db.lock().unwrap();
            time_entries_repo::mark_synced(&conn, entry.id, "wl-1").unwrap();
        }

        let result = update_time_entry_impl(&state, entry.id, None, None, None, Some(120), None);

        assert!(result.is_err(), "a synced entry must never be editable");
        let conn = state.db.lock().unwrap();
        let reloaded = time_entries_repo::get_by_id(&conn, entry.id).unwrap().unwrap();
        assert_eq!(reloaded.duration_seconds, Some(60), "the rejected edit must not have applied");
    }

    #[test]
    fn delete_is_rejected_once_synced() {
        let (state, task_id) = setup();
        let entry = create_manual_entry_impl(&state, task_id, now().to_rfc3339(), None, Some(60), None).unwrap();
        {
            let conn = state.db.lock().unwrap();
            time_entries_repo::mark_synced(&conn, entry.id, "wl-1").unwrap();
        }
        let result = delete_draft_entry_impl(&state, entry.id);
        assert!(result.is_err(), "a synced entry must never be hard-deleted");
    }

    #[test]
    fn delete_is_rejected_for_the_running_timer() {
        let (state, task_id) = setup();
        let running = {
            let conn = state.db.lock().unwrap();
            time_entries_repo::insert_running(&conn, task_id, now(), None).unwrap()
        };

        let result = delete_draft_entry_impl(&state, running.id);

        assert!(result.is_err(), "the running timer must never be deletable");
        let conn = state.db.lock().unwrap();
        assert!(time_entries_repo::get_by_id(&conn, running.id).unwrap().is_some());
    }

    #[test]
    fn delete_succeeds_for_an_unsynced_timer_created_entry() {
        let (state, task_id) = setup();
        let entry_id = {
            let conn = state.db.lock().unwrap();
            let running = time_entries_repo::insert_running(&conn, task_id, now(), None).unwrap();
            time_entries_repo::stop_running(&conn, running.id, now() + Duration::minutes(10), 600).unwrap();
            running.id
        };

        delete_draft_entry_impl(&state, entry_id).unwrap();

        let conn = state.db.lock().unwrap();
        assert!(
            time_entries_repo::get_by_id(&conn, entry_id).unwrap().is_none(),
            "an unsynced entry must be deletable regardless of whether it came from the timer or a manual entry"
        );
    }

    #[test]
    fn delete_succeeds_for_an_unsynced_manual_entry() {
        let (state, task_id) = setup();
        let entry = create_manual_entry_impl(&state, task_id, now().to_rfc3339(), None, Some(60), None).unwrap();
        delete_draft_entry_impl(&state, entry.id).unwrap();

        let conn = state.db.lock().unwrap();
        assert!(time_entries_repo::get_by_id(&conn, entry.id).unwrap().is_none());
    }

    #[test]
    fn create_requires_either_end_or_duration() {
        let (state, task_id) = setup();
        let result = create_manual_entry_impl(&state, task_id, now().to_rfc3339(), None, None, None);
        assert!(result.is_err());
    }
}
