use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};

use super::models::TimeEntry;

fn row_to_time_entry(row: &Row) -> rusqlite::Result<TimeEntry> {
    Ok(TimeEntry {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        task_key: row.get("jira_key")?,
        started_at: row.get("started_at")?,
        ended_at: row.get("ended_at")?,
        duration_seconds: row.get("duration_seconds")?,
        comment: row.get("comment")?,
        is_synced: row.get("is_synced")?,
        jira_worklog_id: row.get("jira_worklog_id")?,
        created_manually: row.get("created_manually")?,
        edited_at: row.get("edited_at")?,
    })
}

const SELECT_JOIN: &str = "SELECT time_entries.*, tasks.jira_key FROM time_entries \
     JOIN tasks ON tasks.id = time_entries.task_id";

pub fn get_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<TimeEntry>> {
    conn.query_row(
        &format!("{SELECT_JOIN} WHERE time_entries.id = ?1"),
        params![id],
        row_to_time_entry,
    )
    .optional()
}

/// The currently running entry, if any — the row with `ended_at IS NULL`. There is at
/// most one by construction (see the partial unique index in the schema).
pub fn get_running(conn: &Connection) -> rusqlite::Result<Option<TimeEntry>> {
    conn.query_row(
        &format!("{SELECT_JOIN} WHERE time_entries.ended_at IS NULL"),
        [],
        row_to_time_entry,
    )
    .optional()
}

pub fn insert_running(
    conn: &Connection,
    task_id: i64,
    started_at: DateTime<Utc>,
    comment: Option<&str>,
) -> rusqlite::Result<TimeEntry> {
    conn.execute(
        "INSERT INTO time_entries (task_id, started_at, comment, created_manually) \
         VALUES (?1, ?2, ?3, 0)",
        params![task_id, started_at, comment],
    )?;
    let id = conn.last_insert_rowid();
    Ok(get_by_id(conn, id)?.expect("row was just inserted"))
}

pub fn stop_running(
    conn: &Connection,
    id: i64,
    ended_at: DateTime<Utc>,
    duration_seconds: i64,
) -> rusqlite::Result<TimeEntry> {
    conn.execute(
        "UPDATE time_entries SET ended_at = ?1, duration_seconds = ?2 WHERE id = ?3",
        params![ended_at, duration_seconds, id],
    )?;
    Ok(get_by_id(conn, id)?.expect("row was just updated"))
}

pub fn insert_manual(
    conn: &Connection,
    task_id: i64,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    duration_seconds: i64,
    comment: Option<&str>,
) -> rusqlite::Result<TimeEntry> {
    conn.execute(
        "INSERT INTO time_entries \
         (task_id, started_at, ended_at, duration_seconds, comment, created_manually) \
         VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        params![task_id, started_at, ended_at, duration_seconds, comment],
    )?;
    let id = conn.last_insert_rowid();
    Ok(get_by_id(conn, id)?.expect("row was just inserted"))
}

/// Fields to change on an existing entry. `None` means "leave unchanged". This backs
/// both "fix the ticket I forgot to switch to" and "adjust start/end/duration".
#[derive(Debug, Default)]
pub struct EntryUpdate {
    pub task_id: Option<i64>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i64>,
    pub comment: Option<Option<String>>,
}

/// Applies `update` to entry `id`, always stamping `edited_at = now`. If the entry was
/// previously pushed to Jira (`jira_worklog_id IS NOT NULL`), `is_synced` is reset to 0
/// so the next Sync sends a PUT (update) rather than skipping it — `is_synced` means
/// "currently in sync with Jira", not "has ever been synced". `jira_worklog_id` itself
/// is left untouched, since that combination (worklog id present + unsynced) is exactly
/// how the sync engine knows to PUT instead of POST.
pub fn update_entry(
    conn: &Connection,
    id: i64,
    update: EntryUpdate,
    now: DateTime<Utc>,
) -> rusqlite::Result<TimeEntry> {
    let current = get_by_id(conn, id)?.expect("caller must verify entry exists");

    let task_id = update.task_id.unwrap_or(current.task_id);
    let started_at = update.started_at.unwrap_or(current.started_at);
    let ended_at = match update.ended_at {
        Some(v) => Some(v),
        None => current.ended_at,
    };
    let duration_seconds = update.duration_seconds.or(current.duration_seconds);
    let comment = update.comment.unwrap_or(current.comment);

    let needs_resync = current.jira_worklog_id.is_some();

    conn.execute(
        "UPDATE time_entries SET \
            task_id = ?1, started_at = ?2, ended_at = ?3, duration_seconds = ?4, \
            comment = ?5, edited_at = ?6, \
            is_synced = CASE WHEN ?7 THEN 0 ELSE is_synced END \
         WHERE id = ?8",
        params![
            task_id,
            started_at,
            ended_at,
            duration_seconds,
            comment,
            now,
            needs_resync,
            id
        ],
    )?;
    Ok(get_by_id(conn, id)?.expect("row was just updated"))
}

pub fn list_entries(
    conn: &Connection,
    task_id: Option<i64>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
) -> rusqlite::Result<Vec<TimeEntry>> {
    let mut sql = format!("{SELECT_JOIN} WHERE 1=1");
    if task_id.is_some() {
        sql.push_str(" AND time_entries.task_id = :task_id");
    }
    if from.is_some() {
        sql.push_str(" AND time_entries.started_at >= :from");
    }
    if to.is_some() {
        sql.push_str(" AND time_entries.started_at <= :to");
    }
    sql.push_str(" ORDER BY time_entries.started_at DESC");

    let mut stmt = conn.prepare(&sql)?;
    let mut named_params: Vec<(&str, &dyn rusqlite::ToSql)> = Vec::new();
    if let Some(v) = &task_id {
        named_params.push((":task_id", v));
    }
    if let Some(v) = &from {
        named_params.push((":from", v));
    }
    if let Some(v) = &to {
        named_params.push((":to", v));
    }
    let rows = stmt.query_map(named_params.as_slice(), row_to_time_entry)?;
    rows.collect()
}

/// Entries that need to be sent to Jira, oldest first — chronological order means an
/// interrupted sync fills the earliest gaps in Jira first, and a retry naturally
/// picks up wherever it left off.
pub fn list_pending_sync(conn: &Connection) -> rusqlite::Result<Vec<TimeEntry>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT_JOIN} WHERE time_entries.ended_at IS NOT NULL AND time_entries.is_synced = 0 \
         ORDER BY time_entries.started_at ASC"
    ))?;
    let rows = stmt.query_map([], row_to_time_entry)?;
    rows.collect()
}

pub fn count_unsynced(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT count(*) FROM time_entries WHERE ended_at IS NOT NULL AND is_synced = 0",
        [],
        |row| row.get(0),
    )
}

pub fn mark_synced(conn: &Connection, id: i64, jira_worklog_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE time_entries SET is_synced = 1, jira_worklog_id = ?1 WHERE id = ?2",
        params![jira_worklog_id, id],
    )?;
    Ok(())
}

/// Hard-deletes an entry, but only ever a manual entry that was never synced — see
/// the guard in `commands::entries::delete_draft_entry`. Anything timer-tracked or
/// ever pushed to Jira is permanent per the spec.
pub fn delete_draft(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM time_entries WHERE id = ?1", params![id])?;
    Ok(())
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

    fn setup_task(conn: &Connection) -> i64 {
        tasks_repo::upsert_favorite_task(conn, "PROJ-1", "Test ticket", now())
            .unwrap()
            .id
    }

    #[test]
    fn editing_a_previously_synced_entry_marks_it_unsynced_again() {
        let conn = open_in_memory().unwrap();
        let task_id = setup_task(&conn);
        let entry =
            insert_manual(&conn, task_id, now(), now(), 3600, None).unwrap();
        mark_synced(&conn, entry.id, "10001").unwrap();

        let updated = update_entry(
            &conn,
            entry.id,
            EntryUpdate {
                duration_seconds: Some(1800),
                ..Default::default()
            },
            now(),
        )
        .unwrap();

        assert!(!updated.is_synced, "edit of a synced entry must flip is_synced back to false");
        assert_eq!(
            updated.jira_worklog_id.as_deref(),
            Some("10001"),
            "worklog id must be preserved so sync knows to PUT, not POST"
        );
        assert_eq!(updated.duration_seconds, Some(1800));
    }

    #[test]
    fn editing_a_never_synced_entry_leaves_is_synced_false() {
        let conn = open_in_memory().unwrap();
        let task_id = setup_task(&conn);
        let entry = insert_manual(&conn, task_id, now(), now(), 3600, None).unwrap();

        let updated = update_entry(
            &conn,
            entry.id,
            EntryUpdate {
                comment: Some(Some("updated".into())),
                ..Default::default()
            },
            now(),
        )
        .unwrap();

        assert!(!updated.is_synced);
        assert!(updated.jira_worklog_id.is_none());
    }

    #[test]
    fn only_one_running_entry_allowed_at_the_db_level() {
        let conn = open_in_memory().unwrap();
        let task_id = setup_task(&conn);
        insert_running(&conn, task_id, now(), None).unwrap();
        let result = insert_running(&conn, task_id, now(), None);
        assert!(result.is_err(), "a second concurrent running row must be rejected");
    }

    #[test]
    fn pending_sync_is_ordered_oldest_first() {
        let conn = open_in_memory().unwrap();
        let task_id = setup_task(&conn);
        let later = now() + chrono::Duration::hours(2);
        insert_manual(&conn, task_id, later, later, 60, None).unwrap();
        insert_manual(&conn, task_id, now(), now(), 60, None).unwrap();

        let pending = list_pending_sync(&conn).unwrap();
        assert_eq!(pending.len(), 2);
        assert!(pending[0].started_at < pending[1].started_at);
    }
}
