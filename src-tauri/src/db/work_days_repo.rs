use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};

use super::models::{WorkBreak, WorkDay};

fn row_to_work_day(row: &Row) -> rusqlite::Result<WorkDay> {
    Ok(WorkDay {
        id: row.get("id")?,
        work_date: row.get("work_date")?,
        started_at: row.get("started_at")?,
        ended_at: row.get("ended_at")?,
    })
}

fn row_to_work_break(row: &Row) -> rusqlite::Result<WorkBreak> {
    Ok(WorkBreak {
        id: row.get("id")?,
        work_day_id: row.get("work_day_id")?,
        started_at: row.get("started_at")?,
        ended_at: row.get("ended_at")?,
    })
}

pub fn get_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<WorkDay>> {
    conn.query_row("SELECT * FROM work_days WHERE id = ?1", params![id], row_to_work_day)
        .optional()
}

/// The currently open workday, if any — the row with `ended_at IS NULL`. There is at
/// most one by construction (see `idx_work_days_single_running`).
pub fn get_running(conn: &Connection) -> rusqlite::Result<Option<WorkDay>> {
    conn.query_row("SELECT * FROM work_days WHERE ended_at IS NULL", [], row_to_work_day)
        .optional()
}

pub fn insert_running(
    conn: &Connection,
    work_date: &str,
    started_at: DateTime<Utc>,
) -> rusqlite::Result<WorkDay> {
    conn.execute(
        "INSERT INTO work_days (work_date, started_at) VALUES (?1, ?2)",
        params![work_date, started_at],
    )?;
    let id = conn.last_insert_rowid();
    Ok(get_by_id(conn, id)?.expect("row was just inserted"))
}

pub fn stop_running(conn: &Connection, id: i64, ended_at: DateTime<Utc>) -> rusqlite::Result<WorkDay> {
    conn.execute("UPDATE work_days SET ended_at = ?1 WHERE id = ?2", params![ended_at, id])?;
    Ok(get_by_id(conn, id)?.expect("row was just updated"))
}

/// All workday rows for a given local calendar date (`YYYY-MM-DD`), oldest first —
/// there can be more than one for split shifts.
pub fn work_days_for_date(conn: &Connection, date: &str) -> rusqlite::Result<Vec<WorkDay>> {
    let mut stmt = conn.prepare("SELECT * FROM work_days WHERE work_date = ?1 ORDER BY started_at ASC")?;
    let rows = stmt.query_map(params![date], row_to_work_day)?;
    rows.collect()
}

pub fn get_break_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<WorkBreak>> {
    conn.query_row("SELECT * FROM work_breaks WHERE id = ?1", params![id], row_to_work_break)
        .optional()
}

/// The currently open break, if any — at most one by construction (see
/// `idx_work_breaks_single_running`).
pub fn get_running_break(conn: &Connection) -> rusqlite::Result<Option<WorkBreak>> {
    conn.query_row("SELECT * FROM work_breaks WHERE ended_at IS NULL", [], row_to_work_break)
        .optional()
}

pub fn insert_break(
    conn: &Connection,
    work_day_id: i64,
    started_at: DateTime<Utc>,
) -> rusqlite::Result<WorkBreak> {
    conn.execute(
        "INSERT INTO work_breaks (work_day_id, started_at) VALUES (?1, ?2)",
        params![work_day_id, started_at],
    )?;
    let id = conn.last_insert_rowid();
    Ok(get_break_by_id(conn, id)?.expect("row was just inserted"))
}

pub fn stop_break(conn: &Connection, id: i64, ended_at: DateTime<Utc>) -> rusqlite::Result<WorkBreak> {
    conn.execute("UPDATE work_breaks SET ended_at = ?1 WHERE id = ?2", params![ended_at, id])?;
    Ok(get_break_by_id(conn, id)?.expect("row was just updated"))
}

pub fn update_break(
    conn: &Connection,
    id: i64,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
) -> rusqlite::Result<WorkBreak> {
    conn.execute(
        "UPDATE work_breaks SET started_at = ?1, ended_at = ?2 WHERE id = ?3",
        params![started_at, ended_at, id],
    )?;
    Ok(get_break_by_id(conn, id)?.expect("row was just updated"))
}

pub fn breaks_for_day(conn: &Connection, work_day_id: i64) -> rusqlite::Result<Vec<WorkBreak>> {
    let mut stmt =
        conn.prepare("SELECT * FROM work_breaks WHERE work_day_id = ?1 ORDER BY started_at ASC")?;
    let rows = stmt.query_map(params![work_day_id], row_to_work_break)?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 11, 9, 0, 0).unwrap()
    }

    #[test]
    fn only_one_running_workday_allowed_at_the_db_level() {
        let conn = open_in_memory().unwrap();
        insert_running(&conn, "2026-08-11", now()).unwrap();
        let result = insert_running(&conn, "2026-08-11", now());
        assert!(result.is_err(), "a second concurrent open workday must be rejected");
    }

    #[test]
    fn only_one_running_break_allowed_at_the_db_level() {
        let conn = open_in_memory().unwrap();
        let day = insert_running(&conn, "2026-08-11", now()).unwrap();
        insert_break(&conn, day.id, now()).unwrap();
        let result = insert_break(&conn, day.id, now());
        assert!(result.is_err(), "a second concurrent open break must be rejected");
    }

    #[test]
    fn stop_running_sets_ended_at() {
        let conn = open_in_memory().unwrap();
        let day = insert_running(&conn, "2026-08-11", now()).unwrap();
        let ended_at = now() + chrono::Duration::hours(8);
        let stopped = stop_running(&conn, day.id, ended_at).unwrap();
        assert_eq!(stopped.ended_at, Some(ended_at));
    }

    #[test]
    fn work_days_for_date_returns_only_matching_rows_oldest_first() {
        let conn = open_in_memory().unwrap();
        let first = insert_running(&conn, "2026-08-11", now()).unwrap();
        stop_running(&conn, first.id, now() + chrono::Duration::hours(1)).unwrap();
        let later_start = now() + chrono::Duration::hours(2);
        let second = insert_running(&conn, "2026-08-11", later_start).unwrap();
        stop_running(&conn, second.id, later_start + chrono::Duration::hours(1)).unwrap();
        insert_running(&conn, "2026-08-12", now()).unwrap();

        let days = work_days_for_date(&conn, "2026-08-11").unwrap();
        assert_eq!(days.len(), 2);
        assert!(days[0].started_at < days[1].started_at);
    }

    #[test]
    fn update_break_overwrites_both_bounds() {
        let conn = open_in_memory().unwrap();
        let day = insert_running(&conn, "2026-08-11", now()).unwrap();
        let brk = insert_break(&conn, day.id, now()).unwrap();
        stop_break(&conn, brk.id, now() + chrono::Duration::minutes(45)).unwrap();

        let corrected_start = now() + chrono::Duration::minutes(5);
        let corrected_end = now() + chrono::Duration::minutes(20);
        let updated = update_break(&conn, brk.id, corrected_start, corrected_end).unwrap();

        assert_eq!(updated.started_at, corrected_start);
        assert_eq!(updated.ended_at, Some(corrected_end));
    }

    #[test]
    fn breaks_for_day_returns_only_that_days_breaks() {
        let conn = open_in_memory().unwrap();
        let day_a = insert_running(&conn, "2026-08-11", now()).unwrap();
        stop_running(&conn, day_a.id, now() + chrono::Duration::hours(1)).unwrap();
        let day_b = insert_running(&conn, "2026-08-12", now()).unwrap();

        let break_a = insert_break(&conn, day_a.id, now()).unwrap();
        stop_break(&conn, break_a.id, now() + chrono::Duration::minutes(15)).unwrap();
        insert_break(&conn, day_b.id, now()).unwrap();

        let breaks = breaks_for_day(&conn, day_a.id).unwrap();
        assert_eq!(breaks.len(), 1);
        assert_eq!(breaks[0].work_day_id, day_a.id);
    }
}
