use chrono::{NaiveDate, Utc};
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::stats::engine::{self, Granularity, IntervalBucket, TicketTotal};

fn parse_date(s: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| AppError::Validation(format!("Invalid date: {s}")))
}

fn get_ticket_stats_impl(state: &AppState, from: String, to: String) -> AppResult<Vec<TicketTotal>> {
    let from = parse_date(&from)?;
    let to = parse_date(&to)?;
    let conn = state.db.lock().unwrap();
    Ok(engine::ticket_totals(&conn, from, to)?)
}

#[tauri::command]
pub fn get_ticket_stats(state: State<'_, AppState>, from: String, to: String) -> AppResult<Vec<TicketTotal>> {
    get_ticket_stats_impl(&state, from, to)
}

fn get_interval_stats_impl(
    state: &AppState,
    from: String,
    to: String,
    granularity: String,
) -> AppResult<Vec<IntervalBucket>> {
    let from = parse_date(&from)?;
    let to = parse_date(&to)?;
    let granularity = Granularity::parse(&granularity)?;
    let conn = state.db.lock().unwrap();
    Ok(engine::interval_stats(&conn, from, to, granularity, Utc::now())?)
}

#[tauri::command]
pub fn get_interval_stats(
    state: State<'_, AppState>,
    from: String,
    to: String,
    granularity: String,
) -> AppResult<Vec<IntervalBucket>> {
    get_interval_stats_impl(&state, from, to, granularity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;

    fn setup() -> AppState {
        AppState::new(open_in_memory().unwrap())
    }

    #[test]
    fn get_ticket_stats_rejects_a_malformed_date() {
        let state = setup();
        let result = get_ticket_stats_impl(&state, "not-a-date".into(), "2026-08-11".into());
        assert!(result.is_err());
    }

    #[test]
    fn get_ticket_stats_returns_empty_for_an_empty_db() {
        let state = setup();
        let result = get_ticket_stats_impl(&state, "2026-08-01".into(), "2026-08-31".into()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn get_interval_stats_rejects_a_malformed_granularity() {
        let state = setup();
        let result =
            get_interval_stats_impl(&state, "2026-08-01".into(), "2026-08-31".into(), "fortnight".into());
        assert!(result.is_err());
    }

    #[test]
    fn get_interval_stats_rejects_a_malformed_date() {
        let state = setup();
        let result = get_interval_stats_impl(&state, "nope".into(), "2026-08-31".into(), "day".into());
        assert!(result.is_err());
    }

    #[test]
    fn get_interval_stats_returns_one_bucket_per_day_for_an_empty_db() {
        let state = setup();
        let result =
            get_interval_stats_impl(&state, "2026-08-01".into(), "2026-08-03".into(), "day".into()).unwrap();
        assert_eq!(result.len(), 3);
    }
}
