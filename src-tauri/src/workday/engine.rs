use chrono::{DateTime, Local, LocalResult, NaiveDate, TimeZone, Utc};
use rusqlite::Connection;
use serde::Serialize;

use crate::db::models::{WorkBreak, WorkDay};
use crate::db::{time_entries_repo, work_days_repo};

#[derive(Debug, thiserror::Error)]
pub enum WorkdayError {
    #[error("no workday is currently running")]
    NotRunning,
    #[error("a workday is already running")]
    AlreadyRunning,
    #[error("a break is already running")]
    BreakAlreadyRunning,
    #[error("no break is currently running")]
    BreakNotRunning,
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailySummary {
    pub date: String,
    pub worked_seconds: i64,
    pub logged_seconds: i64,
    pub diff_seconds: i64,
}

/// The local (single-user desktop) calendar date for a UTC instant — "workday" and
/// "today" are inherently local-day concepts, not UTC-day ones.
pub fn local_date(now: DateTime<Utc>) -> NaiveDate {
    now.with_timezone(&Local).date_naive()
}

pub fn get_running(conn: &Connection) -> Result<Option<WorkDay>, WorkdayError> {
    Ok(work_days_repo::get_running(conn)?)
}

pub fn start_workday(conn: &Connection, now: DateTime<Utc>) -> Result<WorkDay, WorkdayError> {
    if work_days_repo::get_running(conn)?.is_some() {
        return Err(WorkdayError::AlreadyRunning);
    }
    let work_date = local_date(now).format("%Y-%m-%d").to_string();
    Ok(work_days_repo::insert_running(conn, &work_date, now)?)
}

/// Ends the running workday. If a break is currently open, it's closed at the same
/// timestamp first — clocking out naturally ends whatever break you were on, mirroring
/// how starting a new timer auto-stops the previous one (see `timer::engine::start`).
pub fn end_workday(conn: &mut Connection, now: DateTime<Utc>) -> Result<WorkDay, WorkdayError> {
    let running = work_days_repo::get_running(conn)?.ok_or(WorkdayError::NotRunning)?;
    let tx = conn.transaction()?;
    if let Some(open_break) = work_days_repo::get_running_break(&tx)? {
        work_days_repo::stop_break(&tx, open_break.id, now)?;
    }
    let day = work_days_repo::stop_running(&tx, running.id, now)?;
    tx.commit()?;
    Ok(day)
}

/// Unlike the timer, there's no "switch" semantic for a workday/break — starting one
/// while another is already open is a mistake, not a signal to auto-stop the prior
/// one, so this rejects rather than auto-closing.
pub fn start_break(conn: &Connection, now: DateTime<Utc>) -> Result<WorkBreak, WorkdayError> {
    let running = work_days_repo::get_running(conn)?.ok_or(WorkdayError::NotRunning)?;
    if work_days_repo::get_running_break(conn)?.is_some() {
        return Err(WorkdayError::BreakAlreadyRunning);
    }
    Ok(work_days_repo::insert_break(conn, running.id, now)?)
}

pub fn end_break(conn: &Connection, now: DateTime<Utc>) -> Result<WorkBreak, WorkdayError> {
    let running = work_days_repo::get_running_break(conn)?.ok_or(WorkdayError::BreakNotRunning)?;
    Ok(work_days_repo::stop_break(conn, running.id, now)?)
}

/// `now` stands in for a break's `ended_at` while it's still open, so an in-progress
/// break counts against worked time as it happens rather than only once stopped.
fn break_seconds(breaks: &[WorkBreak], now: DateTime<Utc>) -> i64 {
    breaks
        .iter()
        .map(|b| (b.ended_at.unwrap_or(now) - b.started_at).num_seconds().max(0))
        .sum()
}

/// Pure: workday span minus break time. `now` stands in for `ended_at` while the
/// workday itself is still open, for the same live-counts-as-it-happens reason.
pub fn worked_seconds(day: &WorkDay, breaks: &[WorkBreak], now: DateTime<Utc>) -> i64 {
    let span = (day.ended_at.unwrap_or(now) - day.started_at).num_seconds().max(0);
    (span - break_seconds(breaks, now)).max(0)
}

/// Worked and break seconds already banked today from every *other* `work_days`
/// session on `date` — i.e. excluding `current_day_id`. Used to make resuming a
/// workday after an earlier clock-out today (e.g. stepping out for lunch) continue
/// counting from where it left off, instead of the live elapsed display resetting to
/// zero for the new session. Every other same-date row is guaranteed already closed,
/// since at most one `work_days` row can be open at a time and `current_day_id` holds
/// that slot — so `now` here only matters for a still-open break under a closed day,
/// which can't happen either (ending a workday auto-closes its open break).
pub fn prior_today_seconds(
    conn: &Connection,
    date: NaiveDate,
    current_day_id: i64,
    now: DateTime<Utc>,
) -> Result<(i64, i64), WorkdayError> {
    let date_str = date.format("%Y-%m-%d").to_string();
    let days = work_days_repo::work_days_for_date(conn, &date_str)?;
    let mut worked = 0i64;
    let mut breaks_total = 0i64;
    for day in days.iter().filter(|d| d.id != current_day_id) {
        let breaks = work_days_repo::breaks_for_day(conn, day.id)?;
        worked += worked_seconds(day, &breaks, now);
        breaks_total += break_seconds(&breaks, now);
    }
    Ok((worked, breaks_total))
}

/// The local calendar day, expressed as UTC instant bounds `[start, end)`, used to
/// select which `time_entries` rows count as "logged" for that day. Ambiguous local
/// times (a DST fold) resolve to the earlier instant; this is a best-effort choice for
/// a twice-a-year edge case, not a correctness-critical one.
fn local_day_bounds_utc(date: NaiveDate) -> (DateTime<Utc>, DateTime<Utc>) {
    let to_utc = |naive| match Local.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt.with_timezone(&Utc),
        LocalResult::Ambiguous(earliest, _) => earliest.with_timezone(&Utc),
        LocalResult::None => Utc.from_utc_datetime(&naive),
    };
    let start = to_utc(date.and_hms_opt(0, 0, 0).unwrap());
    let end = to_utc((date + chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap());
    (start, end)
}

/// Sums worked time (across every workday session on `date`, i.e. split shifts) and
/// Jira-logged time for that same local day, so the caller can show the gap between
/// actual desk time and time that made it into a ticket.
pub fn daily_summary(
    conn: &Connection,
    date: NaiveDate,
    now: DateTime<Utc>,
) -> Result<DailySummary, WorkdayError> {
    let date_str = date.format("%Y-%m-%d").to_string();
    let days = work_days_repo::work_days_for_date(conn, &date_str)?;
    let mut worked = 0i64;
    for day in &days {
        let breaks = work_days_repo::breaks_for_day(conn, day.id)?;
        worked += worked_seconds(day, &breaks, now);
    }

    let (from, to) = local_day_bounds_utc(date);
    let entries = time_entries_repo::list_entries(conn, None, Some(from), Some(to))?;
    let logged: i64 = entries.iter().filter_map(|e| e.duration_seconds).sum();

    Ok(DailySummary {
        date: date_str,
        worked_seconds: worked,
        logged_seconds: logged,
        diff_seconds: worked - logged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;
    use crate::db::tasks_repo;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 11, 9, 0, 0).unwrap()
    }

    #[test]
    fn starting_a_second_workday_is_rejected() {
        let conn = open_in_memory().unwrap();
        start_workday(&conn, now()).unwrap();
        let result = start_workday(&conn, now());
        assert!(matches!(result, Err(WorkdayError::AlreadyRunning)));
    }

    #[test]
    fn ending_with_no_workday_running_errors() {
        let mut conn = open_in_memory().unwrap();
        let result = end_workday(&mut conn, now());
        assert!(matches!(result, Err(WorkdayError::NotRunning)));
    }

    #[test]
    fn starting_a_second_break_is_rejected() {
        let conn = open_in_memory().unwrap();
        start_workday(&conn, now()).unwrap();
        start_break(&conn, now()).unwrap();
        let result = start_break(&conn, now());
        assert!(matches!(result, Err(WorkdayError::BreakAlreadyRunning)));
    }

    #[test]
    fn starting_a_break_without_a_workday_is_rejected() {
        let conn = open_in_memory().unwrap();
        let result = start_break(&conn, now());
        assert!(matches!(result, Err(WorkdayError::NotRunning)));
    }

    #[test]
    fn ending_the_workday_auto_closes_an_open_break() {
        let mut conn = open_in_memory().unwrap();
        start_workday(&conn, now()).unwrap();
        let brk = start_break(&conn, now() + chrono::Duration::minutes(10)).unwrap();
        assert!(brk.is_running());

        let end_time = now() + chrono::Duration::hours(8);
        end_workday(&mut conn, end_time).unwrap();

        let reloaded = work_days_repo::get_break_by_id(&conn, brk.id).unwrap().unwrap();
        assert_eq!(reloaded.ended_at, Some(end_time), "open break must close at the same instant the day ends");
    }

    #[test]
    fn worked_seconds_subtracts_closed_breaks() {
        let day = WorkDay {
            id: 1,
            work_date: "2026-08-11".into(),
            started_at: now(),
            ended_at: Some(now() + chrono::Duration::hours(8)),
        };
        let breaks = vec![WorkBreak {
            id: 1,
            work_day_id: 1,
            started_at: now() + chrono::Duration::hours(4),
            ended_at: Some(now() + chrono::Duration::hours(4) + chrono::Duration::minutes(30)),
        }];
        let seconds = worked_seconds(&day, &breaks, now());
        assert_eq!(seconds, 8 * 3600 - 30 * 60);
    }

    #[test]
    fn worked_seconds_counts_an_in_progress_break_against_worked_time() {
        let day = WorkDay {
            id: 1,
            work_date: "2026-08-11".into(),
            started_at: now(),
            ended_at: None,
        };
        let breaks = vec![WorkBreak {
            id: 1,
            work_day_id: 1,
            started_at: now() + chrono::Duration::hours(2),
            ended_at: None,
        }];
        let current = now() + chrono::Duration::hours(3);
        let seconds = worked_seconds(&day, &breaks, current);
        assert_eq!(seconds, 2 * 3600, "elapsed 3h minus 1h of still-running break");
    }

    #[test]
    fn daily_summary_sums_across_split_shifts_and_compares_to_jira_logged_time() {
        let conn = open_in_memory().unwrap();
        let task_id = tasks_repo::upsert_favorite_task(&conn, "PROJ-1", "Ticket", now()).unwrap().id;

        // Morning shift: 9:00 - 12:00 local, no break.
        let morning_start = Utc.with_ymd_and_hms(2026, 8, 11, 9, 0, 0).unwrap();
        let day1 = work_days_repo::insert_running(&conn, "2026-08-11", morning_start).unwrap();
        work_days_repo::stop_running(&conn, day1.id, morning_start + chrono::Duration::hours(3)).unwrap();

        // Afternoon shift: 13:00 - 17:00 local.
        let afternoon_start = Utc.with_ymd_and_hms(2026, 8, 11, 13, 0, 0).unwrap();
        let day2 = work_days_repo::insert_running(&conn, "2026-08-11", afternoon_start).unwrap();
        work_days_repo::stop_running(&conn, day2.id, afternoon_start + chrono::Duration::hours(4)).unwrap();

        // 2 hours logged to Jira sometime that day. Deliberately placed near UTC
        // midday (not near a local-midnight boundary) so this assertion holds
        // regardless of the test machine's system timezone.
        let jira_logged_start = Utc.with_ymd_and_hms(2026, 8, 11, 12, 0, 0).unwrap();
        time_entries_repo::insert_manual(
            &conn,
            task_id,
            jira_logged_start,
            jira_logged_start + chrono::Duration::hours(2),
            2 * 3600,
            None,
        )
        .unwrap();

        let date = NaiveDate::from_ymd_opt(2026, 8, 11).unwrap();
        let summary = daily_summary(&conn, date, morning_start + chrono::Duration::hours(10)).unwrap();

        assert_eq!(summary.worked_seconds, 7 * 3600);
        assert_eq!(summary.logged_seconds, 2 * 3600);
        assert_eq!(summary.diff_seconds, 5 * 3600);
    }

    #[test]
    fn prior_today_seconds_is_zero_with_no_earlier_session_today() {
        let conn = open_in_memory().unwrap();
        let day = start_workday(&conn, now()).unwrap();
        let date = local_date(now());

        let (worked, breaks) = prior_today_seconds(&conn, date, day.id, now()).unwrap();
        assert_eq!(worked, 0);
        assert_eq!(breaks, 0);
    }

    #[test]
    fn prior_today_seconds_counts_an_earlier_closed_session_and_its_break() {
        let conn = open_in_memory().unwrap();

        // Morning session: 4h worked, one 30-minute break.
        let morning_start = now();
        let morning = work_days_repo::insert_running(&conn, "2026-08-11", morning_start).unwrap();
        let brk = work_days_repo::insert_break(&conn, morning.id, morning_start + chrono::Duration::hours(1)).unwrap();
        work_days_repo::stop_break(&conn, brk.id, morning_start + chrono::Duration::hours(1) + chrono::Duration::minutes(30)).unwrap();
        work_days_repo::stop_running(&conn, morning.id, morning_start + chrono::Duration::hours(4)).unwrap();

        // Resumed afternoon session — currently open.
        let afternoon_start = morning_start + chrono::Duration::hours(5);
        let afternoon = work_days_repo::insert_running(&conn, "2026-08-11", afternoon_start).unwrap();

        let (worked, breaks) =
            prior_today_seconds(&conn, local_date(morning_start), afternoon.id, afternoon_start).unwrap();

        assert_eq!(worked, 4 * 3600 - 30 * 60, "prior session's worked time, minus its break");
        assert_eq!(breaks, 30 * 60, "prior session's break time");
    }
}
