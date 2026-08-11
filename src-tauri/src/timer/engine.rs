use chrono::{DateTime, Utc};
use rusqlite::Connection;

use crate::db::models::TimeEntry;
use crate::db::time_entries_repo;

/// A running timer older than this is treated as a likely crash/left-overnight case
/// worth surfacing to the user, rather than a normal long-running session.
pub const STALE_THRESHOLD_SECS: i64 = 12 * 60 * 60;

#[derive(Debug, thiserror::Error)]
pub enum TimerError {
    #[error("no timer is currently running")]
    NotRunning,
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
}

pub fn get_running(conn: &Connection) -> Result<Option<TimeEntry>, TimerError> {
    Ok(time_entries_repo::get_running(conn)?)
}

/// Enforces "only one active timer": stops-and-saves any currently running entry as a
/// completed record, then starts the new one — both inside a single transaction, so a
/// concurrent call can't observe or create an intermediate two-running-timers state.
pub fn start(
    conn: &mut Connection,
    task_id: i64,
    comment: Option<String>,
    now: DateTime<Utc>,
) -> Result<TimeEntry, TimerError> {
    let tx = conn.transaction()?;
    if let Some(running) = time_entries_repo::get_running(&tx)? {
        let duration = (now - running.started_at).num_seconds().max(0);
        time_entries_repo::stop_running(&tx, running.id, now, duration)?;
    }
    let entry = time_entries_repo::insert_running(&tx, task_id, now, comment.as_deref())?;
    tx.commit()?;
    Ok(entry)
}

pub fn stop(conn: &Connection, now: DateTime<Utc>) -> Result<TimeEntry, TimerError> {
    let running = time_entries_repo::get_running(conn)?.ok_or(TimerError::NotRunning)?;
    let duration = (now - running.started_at).num_seconds().max(0);
    Ok(time_entries_repo::stop_running(conn, running.id, now, duration)?)
}

/// A running timer counts as stale once it's been going longer than
/// `STALE_THRESHOLD_SECS` — the signal the frontend uses to show a non-blocking
/// "still running — keep going or stop now?" banner on launch, instead of treating
/// every resumed session as a crash that needs a blocking prompt.
pub fn is_stale(entry: &TimeEntry, now: DateTime<Utc>) -> bool {
    entry.is_running() && (now - entry.started_at).num_seconds() > STALE_THRESHOLD_SECS
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

    fn setup_two_tasks(conn: &Connection) -> (i64, i64) {
        let a = tasks_repo::upsert_favorite_task(conn, "PROJ-1", "First", now()).unwrap().id;
        let b = tasks_repo::upsert_favorite_task(conn, "PROJ-2", "Second", now()).unwrap().id;
        (a, b)
    }

    #[test]
    fn starting_a_new_timer_stops_and_saves_the_previous_one() {
        let mut conn = open_in_memory().unwrap();
        let (task_a, task_b) = setup_two_tasks(&conn);

        let first = start(&mut conn, task_a, None, now()).unwrap();
        assert!(first.is_running());

        let later = now() + chrono::Duration::minutes(30);
        let second = start(&mut conn, task_b, None, later).unwrap();

        assert!(second.is_running());
        assert_eq!(second.task_id, task_b);

        let first_reloaded = time_entries_repo::get_by_id(&conn, first.id).unwrap().unwrap();
        assert!(!first_reloaded.is_running(), "switching timers must stop the prior one");
        assert_eq!(first_reloaded.duration_seconds, Some(30 * 60));

        // exactly one running timer overall
        assert_eq!(get_running(&conn).unwrap().unwrap().id, second.id);
    }

    #[test]
    fn stopping_with_no_active_timer_errors() {
        let conn = open_in_memory().unwrap();
        let result = stop(&conn, now());
        assert!(matches!(result, Err(TimerError::NotRunning)));
    }

    #[test]
    fn stop_computes_duration_from_started_at() {
        let mut conn = open_in_memory().unwrap();
        let (task_a, _) = setup_two_tasks(&conn);
        start(&mut conn, task_a, None, now()).unwrap();

        let later = now() + chrono::Duration::seconds(125);
        let stopped = stop(&conn, later).unwrap();
        assert_eq!(stopped.duration_seconds, Some(125));
        assert!(!stopped.is_running());
    }

    #[test]
    fn a_forced_second_running_row_is_rejected_at_the_db_level() {
        let conn = open_in_memory().unwrap();
        let (task_a, task_b) = setup_two_tasks(&conn);
        time_entries_repo::insert_running(&conn, task_a, now(), None).unwrap();
        let result = time_entries_repo::insert_running(&conn, task_b, now(), None);
        assert!(result.is_err(), "unique partial index must reject a second running row");
    }

    #[test]
    fn stale_detection_uses_the_threshold() {
        let mut conn = open_in_memory().unwrap();
        let (task_a, _) = setup_two_tasks(&conn);
        let entry = start(&mut conn, task_a, None, now()).unwrap();

        let just_under = now() + chrono::Duration::seconds(STALE_THRESHOLD_SECS - 1);
        assert!(!is_stale(&entry, just_under));

        let just_over = now() + chrono::Duration::seconds(STALE_THRESHOLD_SECS + 1);
        assert!(is_stale(&entry, just_over));
    }
}
