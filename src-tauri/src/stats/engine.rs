use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rusqlite::Connection;
use serde::Serialize;

use crate::db::models::TimeEntry;
use crate::db::{time_entries_repo, work_days_repo};
use crate::workday::engine as workday_engine;

#[derive(Debug, thiserror::Error)]
pub enum StatsError {
    #[error("invalid granularity: {0}")]
    InvalidGranularity(String),
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularity {
    Day,
    Week,
    Month,
}

impl Granularity {
    pub fn parse(s: &str) -> Result<Self, StatsError> {
        match s {
            "day" => Ok(Granularity::Day),
            "week" => Ok(Granularity::Week),
            "month" => Ok(Granularity::Month),
            other => Err(StatsError::InvalidGranularity(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketTotal {
    pub task_id: i64,
    pub task_key: String,
    pub task_summary: String,
    pub total_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketSeconds {
    pub task_id: i64,
    pub task_key: String,
    pub seconds: i64,
}

/// One bucket of the interval breakdown. `period_start`/`period_end` are inclusive
/// local calendar dates (`YYYY-MM-DD`) — formatting them into an axis label is a view
/// concern, left to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntervalBucket {
    pub period_start: String,
    pub period_end: String,
    pub tickets: Vec<TicketSeconds>,
    pub break_seconds: i64,
}

/// Sums each entry's `duration_seconds` by `task_id`, skipping the currently-running
/// entry (`duration_seconds` is `None` until it's stopped) — the same rule
/// `workday::engine::range_summary` uses for its "logged" total.
fn sum_by_task(entries: &[TimeEntry]) -> BTreeMap<i64, (String, String, i64)> {
    let mut totals: BTreeMap<i64, (String, String, i64)> = BTreeMap::new();
    for entry in entries {
        let Some(seconds) = entry.duration_seconds else { continue };
        totals
            .entry(entry.task_id)
            .and_modify(|(_, _, total)| *total += seconds)
            .or_insert_with(|| (entry.task_key.clone(), entry.task_summary.clone(), seconds));
    }
    totals
}

/// Total time logged per ticket over the inclusive local calendar range `[from, to]`,
/// sorted with the most-logged ticket first.
pub fn ticket_totals(
    conn: &Connection,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<TicketTotal>, StatsError> {
    let (from_utc, to_utc) = workday_engine::local_range_bounds_utc(from, to);
    let entries = time_entries_repo::list_entries(conn, None, Some(from_utc), Some(to_utc))?;

    let mut totals: Vec<TicketTotal> = sum_by_task(&entries)
        .into_iter()
        .map(|(task_id, (task_key, task_summary, total_seconds))| TicketTotal {
            task_id,
            task_key,
            task_summary,
            total_seconds,
        })
        .collect();
    totals.sort_by(|a, b| b.total_seconds.cmp(&a.total_seconds));
    Ok(totals)
}

/// Calendar-aligned `(start, end)` bucket bounds covering `[from, to]`. Week/month
/// buckets align to `week_start`/`month_start` regardless of exactly where `from`
/// falls within that period — e.g. a range starting mid-week still gets one full
/// week-aligned bucket, matching how calendars are normally read.
fn buckets_for(granularity: Granularity, from: NaiveDate, to: NaiveDate) -> Vec<(NaiveDate, NaiveDate)> {
    let mut buckets = Vec::new();
    match granularity {
        Granularity::Day => {
            let mut date = from;
            while date <= to {
                buckets.push((date, date));
                date += chrono::Duration::days(1);
            }
        }
        Granularity::Week => {
            let mut start = workday_engine::week_start(from);
            while start <= to {
                buckets.push((start, start + chrono::Duration::days(6)));
                start += chrono::Duration::days(7);
            }
        }
        Granularity::Month => {
            let mut start = workday_engine::month_start(from);
            while start <= to {
                let next_month = if start.month() == 12 {
                    NaiveDate::from_ymd_opt(start.year() + 1, 1, 1)
                } else {
                    NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1)
                }
                .expect("month arithmetic within a valid calendar always produces a valid date");
                buckets.push((start, next_month - chrono::Duration::days(1)));
                start = next_month;
            }
        }
    }
    buckets
}

/// Break time logged across every `work_days` session on any local date in
/// `[from, to]`; `now` stands in for a still-open break's `ended_at`, the same
/// live-counts convention as `workday::engine::range_summary`.
fn break_seconds_in_range(
    conn: &Connection,
    from: NaiveDate,
    to: NaiveDate,
    now: DateTime<Utc>,
) -> Result<i64, StatsError> {
    let mut total = 0i64;
    let mut date = from;
    while date <= to {
        let date_str = date.format("%Y-%m-%d").to_string();
        for day in work_days_repo::work_days_for_date(conn, &date_str)? {
            let breaks = work_days_repo::breaks_for_day(conn, day.id)?;
            total += workday_engine::break_seconds(&breaks, now);
        }
        date += chrono::Duration::days(1);
    }
    Ok(total)
}

/// Per-ticket time and break time, bucketed by `granularity`, across the inclusive
/// local calendar range `[from, to]`.
pub fn interval_stats(
    conn: &Connection,
    from: NaiveDate,
    to: NaiveDate,
    granularity: Granularity,
    now: DateTime<Utc>,
) -> Result<Vec<IntervalBucket>, StatsError> {
    let mut result = Vec::new();
    for (bucket_from, bucket_to) in buckets_for(granularity, from, to) {
        let (from_utc, to_utc) = workday_engine::local_range_bounds_utc(bucket_from, bucket_to);
        let entries = time_entries_repo::list_entries(conn, None, Some(from_utc), Some(to_utc))?;

        let mut tickets: Vec<TicketSeconds> = sum_by_task(&entries)
            .into_iter()
            .map(|(task_id, (task_key, _summary, seconds))| TicketSeconds { task_id, task_key, seconds })
            .collect();
        tickets.sort_by(|a, b| b.seconds.cmp(&a.seconds));

        result.push(IntervalBucket {
            period_start: bucket_from.format("%Y-%m-%d").to_string(),
            period_end: bucket_to.format("%Y-%m-%d").to_string(),
            tickets,
            break_seconds: break_seconds_in_range(conn, bucket_from, bucket_to, now)?,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;
    use crate::db::tasks_repo;
    use chrono::TimeZone;

    // Noon UTC, comfortably clear of local-midnight boundaries regardless of the test
    // machine's system timezone (same convention as `workday::engine`'s tests).
    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 11, 12, 0, 0).unwrap()
    }

    #[test]
    fn granularity_parse_accepts_known_values() {
        assert_eq!(Granularity::parse("day").unwrap(), Granularity::Day);
        assert_eq!(Granularity::parse("week").unwrap(), Granularity::Week);
        assert_eq!(Granularity::parse("month").unwrap(), Granularity::Month);
    }

    #[test]
    fn granularity_parse_rejects_an_unknown_string() {
        assert!(matches!(Granularity::parse("fortnight"), Err(StatsError::InvalidGranularity(_))));
    }

    #[test]
    fn ticket_totals_sums_multiple_entries_per_ticket_and_sorts_descending() {
        let conn = open_in_memory().unwrap();
        let task_a = tasks_repo::upsert_favorite_task(&conn, "PROJ-1", "Ticket A", now()).unwrap().id;
        let task_b = tasks_repo::upsert_favorite_task(&conn, "PROJ-2", "Ticket B", now()).unwrap().id;

        time_entries_repo::insert_manual(&conn, task_a, now(), now() + chrono::Duration::hours(1), 3600, None).unwrap();
        time_entries_repo::insert_manual(&conn, task_a, now(), now() + chrono::Duration::minutes(30), 1800, None).unwrap();
        time_entries_repo::insert_manual(&conn, task_b, now(), now() + chrono::Duration::hours(3), 10800, None).unwrap();

        let from = NaiveDate::from_ymd_opt(2026, 8, 10).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let totals = ticket_totals(&conn, from, to).unwrap();

        assert_eq!(totals.len(), 2);
        assert_eq!(totals[0].task_key, "PROJ-2");
        assert_eq!(totals[0].total_seconds, 10800);
        assert_eq!(totals[1].task_key, "PROJ-1");
        assert_eq!(totals[1].total_seconds, 5400);
    }

    #[test]
    fn ticket_totals_excludes_the_currently_running_entry() {
        let conn = open_in_memory().unwrap();
        let task_id = tasks_repo::upsert_favorite_task(&conn, "PROJ-1", "Ticket", now()).unwrap().id;
        time_entries_repo::insert_running(&conn, task_id, now(), None).unwrap();

        let from = NaiveDate::from_ymd_opt(2026, 8, 10).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let totals = ticket_totals(&conn, from, to).unwrap();

        assert!(totals.is_empty());
    }

    #[test]
    fn interval_stats_day_granularity_produces_one_bucket_per_calendar_day() {
        let conn = open_in_memory().unwrap();
        let from = NaiveDate::from_ymd_opt(2026, 8, 10).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();

        let buckets = interval_stats(&conn, from, to, Granularity::Day, now()).unwrap();

        assert_eq!(buckets.len(), 3);
        assert_eq!(buckets[0].period_start, "2026-08-10");
        assert_eq!(buckets[0].period_end, "2026-08-10");
        assert_eq!(buckets[2].period_start, "2026-08-12");
    }

    #[test]
    fn interval_stats_week_granularity_groups_entries_within_the_same_calendar_week() {
        let conn = open_in_memory().unwrap();
        let task_id = tasks_repo::upsert_favorite_task(&conn, "PROJ-1", "Ticket", now()).unwrap().id;

        // 2026-08-10 is a Monday; 2026-08-12 falls in the same Mon–Sun week.
        let day_a = Utc.with_ymd_and_hms(2026, 8, 10, 12, 0, 0).unwrap();
        let day_b = Utc.with_ymd_and_hms(2026, 8, 12, 12, 0, 0).unwrap();
        time_entries_repo::insert_manual(&conn, task_id, day_a, day_a + chrono::Duration::hours(1), 3600, None).unwrap();
        time_entries_repo::insert_manual(&conn, task_id, day_b, day_b + chrono::Duration::hours(2), 7200, None).unwrap();

        let from = NaiveDate::from_ymd_opt(2026, 8, 10).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let buckets = interval_stats(&conn, from, to, Granularity::Week, day_b).unwrap();

        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].period_start, "2026-08-10");
        assert_eq!(buckets[0].tickets[0].seconds, 3600 + 7200);
    }

    #[test]
    fn interval_stats_month_granularity_splits_entries_across_a_month_boundary() {
        let conn = open_in_memory().unwrap();
        let task_id = tasks_repo::upsert_favorite_task(&conn, "PROJ-1", "Ticket", now()).unwrap().id;

        let aug_entry = Utc.with_ymd_and_hms(2026, 8, 30, 12, 0, 0).unwrap();
        let sep_entry = Utc.with_ymd_and_hms(2026, 9, 2, 12, 0, 0).unwrap();
        time_entries_repo::insert_manual(&conn, task_id, aug_entry, aug_entry + chrono::Duration::hours(1), 3600, None).unwrap();
        time_entries_repo::insert_manual(&conn, task_id, sep_entry, sep_entry + chrono::Duration::hours(2), 7200, None).unwrap();

        let from = NaiveDate::from_ymd_opt(2026, 8, 30).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 9, 2).unwrap();
        let buckets = interval_stats(&conn, from, to, Granularity::Month, sep_entry).unwrap();

        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].period_start, "2026-08-01");
        assert_eq!(buckets[0].tickets[0].seconds, 3600);
        assert_eq!(buckets[1].period_start, "2026-09-01");
        assert_eq!(buckets[1].tickets[0].seconds, 7200);
    }

    #[test]
    fn interval_stats_attributes_break_time_to_the_correct_bucket_including_a_live_break() {
        let conn = open_in_memory().unwrap();
        let day_start = Utc.with_ymd_and_hms(2026, 8, 11, 9, 0, 0).unwrap();
        let day = work_days_repo::insert_running(&conn, "2026-08-11", day_start).unwrap();
        work_days_repo::insert_break(&conn, day.id, day_start + chrono::Duration::hours(1)).unwrap();

        let now = day_start + chrono::Duration::hours(1) + chrono::Duration::minutes(20);
        let date = NaiveDate::from_ymd_opt(2026, 8, 11).unwrap();
        let buckets = interval_stats(&conn, date, date, Granularity::Day, now).unwrap();

        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].break_seconds, 20 * 60);
    }
}
