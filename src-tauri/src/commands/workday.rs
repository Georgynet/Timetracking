use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use tauri::State;

use crate::db::models::{WorkBreak, WorkDay};
use crate::db::work_days_repo;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::workday::engine::{self, DailySummary, RangeSummary};

fn parse_dt(s: &str) -> AppResult<DateTime<Utc>> {
    s.parse::<DateTime<Utc>>()
        .map_err(|_| AppError::Validation(format!("Invalid date/time: {s}")))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkdayStatusDto {
    #[serde(flatten)]
    pub day: WorkDay,
    pub breaks: Vec<WorkBreak>,
    pub is_on_break: bool,
    /// Worked/break seconds already banked today from earlier, already-closed
    /// sessions (see `engine::prior_today_seconds`) — lets the frontend show a live
    /// elapsed clock that resumes from today's running total instead of resetting to
    /// zero each time a workday is stopped and restarted on the same day.
    pub prior_worked_seconds_today: i64,
    pub prior_break_seconds_today: i64,
}

fn get_active_workday_impl(state: &AppState) -> AppResult<Option<WorkdayStatusDto>> {
    let conn = state.db.lock().unwrap();
    let Some(day) = engine::get_running(&conn)? else {
        return Ok(None);
    };
    let breaks = work_days_repo::breaks_for_day(&conn, day.id)?;
    let is_on_break = breaks.iter().any(|b| b.is_running());
    let date = NaiveDate::parse_from_str(&day.work_date, "%Y-%m-%d")
        .expect("work_date is always YYYY-MM-DD, set by engine::start_workday");
    let (prior_worked_seconds_today, prior_break_seconds_today) =
        engine::prior_today_seconds(&conn, date, day.id, Utc::now())?;
    Ok(Some(WorkdayStatusDto {
        day,
        breaks,
        is_on_break,
        prior_worked_seconds_today,
        prior_break_seconds_today,
    }))
}

#[tauri::command]
pub fn get_active_workday(state: State<'_, AppState>) -> AppResult<Option<WorkdayStatusDto>> {
    get_active_workday_impl(&state)
}

fn start_workday_impl(state: &AppState) -> AppResult<WorkDay> {
    let conn = state.db.lock().unwrap();
    Ok(engine::start_workday(&conn, Utc::now())?)
}

#[tauri::command]
pub fn start_workday(state: State<'_, AppState>) -> AppResult<WorkDay> {
    start_workday_impl(&state)
}

fn end_workday_impl(state: &AppState) -> AppResult<WorkDay> {
    let mut conn = state.db.lock().unwrap();
    Ok(engine::end_workday(&mut conn, Utc::now())?)
}

#[tauri::command]
pub fn end_workday(state: State<'_, AppState>) -> AppResult<WorkDay> {
    end_workday_impl(&state)
}

fn start_break_impl(state: &AppState) -> AppResult<WorkBreak> {
    let conn = state.db.lock().unwrap();
    Ok(engine::start_break(&conn, Utc::now())?)
}

#[tauri::command]
pub fn start_break(state: State<'_, AppState>) -> AppResult<WorkBreak> {
    start_break_impl(&state)
}

fn end_break_impl(state: &AppState) -> AppResult<WorkBreak> {
    let conn = state.db.lock().unwrap();
    Ok(engine::end_break(&conn, Utc::now())?)
}

#[tauri::command]
pub fn end_break(state: State<'_, AppState>) -> AppResult<WorkBreak> {
    end_break_impl(&state)
}

fn update_break_impl(
    state: &AppState,
    id: i64,
    started_at: String,
    ended_at: String,
) -> AppResult<WorkBreak> {
    let started_at = parse_dt(&started_at)?;
    let ended_at = parse_dt(&ended_at)?;
    let conn = state.db.lock().unwrap();
    Ok(engine::update_break(&conn, id, started_at, ended_at, Utc::now())?)
}

#[tauri::command]
pub fn update_break(
    state: State<'_, AppState>,
    id: i64,
    started_at: String,
    ended_at: String,
) -> AppResult<WorkBreak> {
    update_break_impl(&state, id, started_at, ended_at)
}

/// `date` (`YYYY-MM-DD`) defaults to today in the local timezone when omitted — see
/// `workday::engine::local_date`.
fn get_daily_summary_impl(state: &AppState, date: Option<String>) -> AppResult<DailySummary> {
    let now = Utc::now();
    let date = match date {
        Some(s) => NaiveDate::parse_from_str(&s, "%Y-%m-%d")
            .map_err(|_| AppError::Validation(format!("Invalid date: {s}")))?,
        None => engine::local_date(now),
    };
    let conn = state.db.lock().unwrap();
    Ok(engine::daily_summary(&conn, date, now)?)
}

#[tauri::command]
pub fn get_daily_summary(state: State<'_, AppState>, date: Option<String>) -> AppResult<DailySummary> {
    get_daily_summary_impl(&state, date)
}

/// Week-to-date: from the Monday of the current local week through today.
fn get_week_summary_impl(state: &AppState) -> AppResult<RangeSummary> {
    let now = Utc::now();
    let today = engine::local_date(now);
    let conn = state.db.lock().unwrap();
    Ok(engine::range_summary(&conn, engine::week_start(today), today, now)?)
}

#[tauri::command]
pub fn get_week_summary(state: State<'_, AppState>) -> AppResult<RangeSummary> {
    get_week_summary_impl(&state)
}

/// Month-to-date: from the 1st of the current local month through today.
fn get_month_summary_impl(state: &AppState) -> AppResult<RangeSummary> {
    let now = Utc::now();
    let today = engine::local_date(now);
    let conn = state.db.lock().unwrap();
    Ok(engine::range_summary(&conn, engine::month_start(today), today, now)?)
}

#[tauri::command]
pub fn get_month_summary(state: State<'_, AppState>) -> AppResult<RangeSummary> {
    get_month_summary_impl(&state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;
    use chrono::TimeZone;

    fn setup() -> AppState {
        AppState::new(open_in_memory().unwrap())
    }

    fn fixed_past() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2020, 1, 1, 9, 0, 0).unwrap()
    }

    #[test]
    fn no_active_workday_returns_none() {
        let state = setup();
        assert!(get_active_workday_impl(&state).unwrap().is_none());
    }

    #[test]
    fn starting_and_ending_a_workday_round_trips_through_the_status_dto() {
        let state = setup();
        start_workday_impl(&state).unwrap();

        let status = get_active_workday_impl(&state).unwrap().unwrap();
        assert!(status.day.ended_at.is_none());
        assert!(!status.is_on_break);
        assert!(status.breaks.is_empty());
        assert_eq!(status.prior_worked_seconds_today, 0);
        assert_eq!(status.prior_break_seconds_today, 0);

        end_workday_impl(&state).unwrap();
        assert!(get_active_workday_impl(&state).unwrap().is_none());
    }

    #[test]
    fn starting_a_second_workday_is_rejected() {
        let state = setup();
        start_workday_impl(&state).unwrap();
        let result = start_workday_impl(&state);
        assert!(result.is_err(), "a second concurrent workday must be rejected");
    }

    #[test]
    fn resuming_a_workday_after_ending_carries_forward_prior_worked_time() {
        let state = setup();
        start_workday_impl(&state).unwrap();
        let ended = end_workday_impl(&state).unwrap();
        let expected_prior = (ended.ended_at.unwrap() - ended.started_at).num_seconds();

        start_workday_impl(&state).unwrap();
        let status = get_active_workday_impl(&state).unwrap().unwrap();

        assert_eq!(
            status.prior_worked_seconds_today, expected_prior,
            "restarting on the same day must carry forward the ended session's worked time"
        );
    }

    #[test]
    fn starting_a_break_flips_is_on_break_in_the_status_dto() {
        let state = setup();
        start_workday_impl(&state).unwrap();
        start_break_impl(&state).unwrap();

        let status = get_active_workday_impl(&state).unwrap().unwrap();
        assert!(status.is_on_break);
        assert_eq!(status.breaks.len(), 1);

        end_break_impl(&state).unwrap();
        let status = get_active_workday_impl(&state).unwrap().unwrap();
        assert!(!status.is_on_break);
    }

    #[test]
    fn ending_a_workday_with_an_open_break_closes_it_too() {
        let state = setup();
        start_workday_impl(&state).unwrap();
        start_break_impl(&state).unwrap();

        end_workday_impl(&state).unwrap();

        let conn = state.db.lock().unwrap();
        let day = work_days_repo::get_running(&conn).unwrap();
        assert!(day.is_none());
    }

    #[test]
    fn daily_summary_defaults_to_today_when_no_date_given() {
        let state = setup();
        let summary = get_daily_summary_impl(&state, None).unwrap();
        assert_eq!(summary.date, engine::local_date(Utc::now()).format("%Y-%m-%d").to_string());
    }

    #[test]
    fn daily_summary_rejects_a_malformed_date() {
        let state = setup();
        let result = get_daily_summary_impl(&state, Some("not-a-date".into()));
        assert!(result.is_err());
    }

    #[test]
    fn week_summary_spans_from_monday_through_today() {
        let state = setup();
        let today = engine::local_date(Utc::now());
        let summary = get_week_summary_impl(&state).unwrap();
        assert_eq!(summary.from, engine::week_start(today).format("%Y-%m-%d").to_string());
        assert_eq!(summary.to, today.format("%Y-%m-%d").to_string());
        assert_eq!(summary.worked_seconds, 0);
    }

    #[test]
    fn update_break_corrects_a_completed_breaks_bounds_through_the_command() {
        let state = setup();
        // Fixed, long-past timestamps and a workday that's never been ended, so the
        // command's internal `Utc::now()` upper bound is trivially satisfied
        // regardless of how fast this test happens to run.
        let brk = {
            let conn = state.db.lock().unwrap();
            let day = work_days_repo::insert_running(&conn, "2020-01-01", fixed_past()).unwrap();
            let brk = work_days_repo::insert_break(&conn, day.id, fixed_past() + chrono::Duration::minutes(10))
                .unwrap();
            work_days_repo::stop_break(&conn, brk.id, fixed_past() + chrono::Duration::hours(3)).unwrap(); // forgot to stop it in time
            brk
        };

        let corrected_start = fixed_past() + chrono::Duration::minutes(10);
        let corrected_end = fixed_past() + chrono::Duration::minutes(25);
        let updated =
            update_break_impl(&state, brk.id, corrected_start.to_rfc3339(), corrected_end.to_rfc3339())
                .unwrap();

        assert_eq!(updated.ended_at, Some(corrected_end));
    }

    #[test]
    fn update_break_rejects_a_malformed_timestamp() {
        let state = setup();
        let brk = {
            let conn = state.db.lock().unwrap();
            let day = work_days_repo::insert_running(&conn, "2020-01-01", fixed_past()).unwrap();
            let brk = work_days_repo::insert_break(&conn, day.id, fixed_past()).unwrap();
            work_days_repo::stop_break(&conn, brk.id, fixed_past() + chrono::Duration::minutes(30)).unwrap();
            brk
        };

        let result = update_break_impl(&state, brk.id, "not-a-date".into(), Utc::now().to_rfc3339());
        assert!(result.is_err());
    }

    #[test]
    fn month_summary_spans_from_the_1st_through_today() {
        let state = setup();
        let today = engine::local_date(Utc::now());
        let summary = get_month_summary_impl(&state).unwrap();
        assert_eq!(summary.from, engine::month_start(today).format("%Y-%m-%d").to_string());
        assert_eq!(summary.to, today.format("%Y-%m-%d").to_string());
        assert_eq!(summary.worked_seconds, 0);
    }
}
