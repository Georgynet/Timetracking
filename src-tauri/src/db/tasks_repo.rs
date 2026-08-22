use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};

use super::models::Task;

/// Every `tasks` column plus `last_tracked_at`, the newest `time_entries.started_at`
/// filed against the row. A correlated subquery rather than a join + GROUP BY: the
/// task list is small, and this keeps `SELECT *` semantics for callers that just want
/// the row back after a write.
const SELECT_TASK: &str = "SELECT tasks.*, \
     (SELECT MAX(started_at) FROM time_entries WHERE time_entries.task_id = tasks.id) \
     AS last_tracked_at FROM tasks";

fn row_to_task(row: &Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get("id")?,
        jira_key: row.get("jira_key")?,
        summary: row.get("summary")?,
        is_favorite: row.get("is_favorite")?,
        is_assigned_to_me: row.get("is_assigned_to_me")?,
        is_in_current_sprint: row.get("is_in_current_sprint")?,
        last_synced_at: row.get("last_synced_at")?,
        last_tracked_at: row.get("last_tracked_at")?,
    })
}

pub fn get_task_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<Task>> {
    conn.query_row(&format!("{SELECT_TASK} WHERE id = ?1"), params![id], row_to_task)
        .optional()
}

pub fn get_task_by_key(conn: &Connection, jira_key: &str) -> rusqlite::Result<Option<Task>> {
    conn.query_row(
        &format!("{SELECT_TASK} WHERE jira_key = ?1"),
        params![jira_key],
        row_to_task,
    )
    .optional()
}

pub fn list_my_tasks(conn: &Connection) -> rusqlite::Result<Vec<Task>> {
    let mut stmt = conn.prepare(
        &format!("{SELECT_TASK} WHERE is_assigned_to_me = 1 ORDER BY jira_key"),
    )?;
    let rows = stmt.query_map([], row_to_task)?;
    rows.collect()
}

pub fn list_favorite_tasks(conn: &Connection) -> rusqlite::Result<Vec<Task>> {
    let mut stmt = conn.prepare(&format!("{SELECT_TASK} WHERE is_favorite = 1 ORDER BY jira_key"))?;
    let rows = stmt.query_map([], row_to_task)?;
    rows.collect()
}

/// Clears `is_assigned_to_me` (and, since it's only ever meaningful alongside it,
/// `is_in_current_sprint`) on every task, ahead of re-populating both from a fresh "my
/// tasks" fetch. Never touches `is_favorite` — a ticket can be both, either, or
/// neither, and favorites are managed independently.
pub fn reset_assigned_to_me(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE tasks SET is_assigned_to_me = 0, is_in_current_sprint = 0
         WHERE is_assigned_to_me = 1",
        [],
    )?;
    Ok(())
}

pub fn upsert_assigned_task(
    conn: &Connection,
    jira_key: &str,
    summary: &str,
    is_in_current_sprint: bool,
    now: DateTime<Utc>,
) -> rusqlite::Result<Task> {
    conn.execute(
        "INSERT INTO tasks (jira_key, summary, is_assigned_to_me, is_in_current_sprint, last_synced_at)
         VALUES (?1, ?2, 1, ?3, ?4)
         ON CONFLICT(jira_key) DO UPDATE SET
            summary = excluded.summary,
            is_assigned_to_me = 1,
            is_in_current_sprint = excluded.is_in_current_sprint,
            last_synced_at = excluded.last_synced_at",
        params![jira_key, summary, is_in_current_sprint, now],
    )?;
    Ok(get_task_by_key(conn, jira_key)?.expect("row was just upserted"))
}

/// Inserts a task row if it doesn't exist yet, or refreshes `summary`/`last_synced_at`
/// otherwise. Never sets `is_favorite`/`is_assigned_to_me` — for one-off ticket lookups
/// (e.g. logging time against a ticket via manual entry) that shouldn't quietly add the
/// ticket to Favorites.
pub fn upsert_task(conn: &Connection, jira_key: &str, summary: &str, now: DateTime<Utc>) -> rusqlite::Result<Task> {
    conn.execute(
        "INSERT INTO tasks (jira_key, summary, last_synced_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(jira_key) DO UPDATE SET
            summary = excluded.summary,
            last_synced_at = excluded.last_synced_at",
        params![jira_key, summary, now],
    )?;
    Ok(get_task_by_key(conn, jira_key)?.expect("row was just upserted"))
}

pub fn upsert_favorite_task(
    conn: &Connection,
    jira_key: &str,
    summary: &str,
    now: DateTime<Utc>,
) -> rusqlite::Result<Task> {
    conn.execute(
        "INSERT INTO tasks (jira_key, summary, is_favorite, last_synced_at)
         VALUES (?1, ?2, 1, ?3)
         ON CONFLICT(jira_key) DO UPDATE SET
            summary = excluded.summary,
            is_favorite = 1,
            last_synced_at = excluded.last_synced_at",
        params![jira_key, summary, now],
    )?;
    Ok(get_task_by_key(conn, jira_key)?.expect("row was just upserted"))
}

/// Clears the favorite flag. Never deletes the row — history/foreign keys may still
/// reference it, and the spec requires time-entry history to be permanent.
pub fn remove_favorite(conn: &Connection, task_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE tasks SET is_favorite = 0 WHERE id = ?1",
        params![task_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 10, 12, 0, 0).unwrap()
    }

    #[test]
    fn last_tracked_at_follows_the_newest_time_entry() {
        use crate::db::time_entries_repo;

        let conn = open_in_memory().unwrap();
        let task = upsert_favorite_task(&conn, "PROJ-1", "Ticket", now()).unwrap();
        assert!(
            get_task_by_id(&conn, task.id).unwrap().unwrap().last_tracked_at.is_none(),
            "a never-tracked ticket has no last-tracked time"
        );

        let later = now() + chrono::Duration::hours(3);
        time_entries_repo::insert_manual(&conn, task.id, now(), now(), 60, None).unwrap();
        time_entries_repo::insert_manual(&conn, task.id, later, later, 60, None).unwrap();

        let reloaded = get_task_by_id(&conn, task.id).unwrap().unwrap();
        assert_eq!(reloaded.last_tracked_at, Some(later), "the newest entry wins");
    }

    #[test]
    fn refresh_never_touches_favorite_flag() {
        let conn = open_in_memory().unwrap();
        upsert_favorite_task(&conn, "TEAM-1", "Daily meetings", now()).unwrap();
        upsert_assigned_task(&conn, "TEAM-1", "Daily meetings", true, now()).unwrap();

        let task = get_task_by_key(&conn, "TEAM-1").unwrap().unwrap();
        assert!(task.is_favorite);
        assert!(task.is_assigned_to_me);
        assert!(task.is_in_current_sprint);

        reset_assigned_to_me(&conn).unwrap();
        let task = get_task_by_key(&conn, "TEAM-1").unwrap().unwrap();
        assert!(task.is_favorite, "favorite flag must survive a my-tasks refresh");
        assert!(!task.is_assigned_to_me);
        assert!(!task.is_in_current_sprint);
    }

    #[test]
    fn upsert_task_never_sets_favorite_or_assigned() {
        let conn = open_in_memory().unwrap();
        let task = upsert_task(&conn, "PROJ-1", "Ad-hoc code review", now()).unwrap();
        assert!(!task.is_favorite);
        assert!(!task.is_assigned_to_me);

        // Re-running it (e.g. logging a second one-off entry against the same ticket)
        // must not flip either flag on either.
        let task = upsert_task(&conn, "PROJ-1", "Ad-hoc code review", now()).unwrap();
        assert!(!task.is_favorite);
        assert!(!task.is_assigned_to_me);
    }

    #[test]
    fn upsert_task_does_not_clear_an_existing_favorite_flag() {
        let conn = open_in_memory().unwrap();
        upsert_favorite_task(&conn, "TEAM-1", "Daily meetings", now()).unwrap();

        let task = upsert_task(&conn, "TEAM-1", "Daily meetings", now()).unwrap();
        assert!(task.is_favorite, "upsert_task must not un-favorite an already-favorited ticket");
    }

    #[test]
    fn remove_favorite_does_not_delete_the_row() {
        let conn = open_in_memory().unwrap();
        let task = upsert_favorite_task(&conn, "PROJ-1", "Some ticket", now()).unwrap();
        remove_favorite(&conn, task.id).unwrap();

        let task = get_task_by_key(&conn, "PROJ-1").unwrap();
        assert!(task.is_some(), "row must still exist");
        assert!(!task.unwrap().is_favorite);
    }
}
