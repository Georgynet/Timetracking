//! Which background reminders are due right now. Pure apart from `load_inputs`, so the
//! rules can be tested without a clock or a notification center: the caller passes the
//! current state, the settings, `now`, and a `ReminderMemory` that remembers what was
//! already shown so each reminder fires once per occurrence instead of every tick.

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::Connection;

use super::settings::NotificationSettings;
use super::Notice;
use crate::db::{time_entries_repo, work_days_repo};
use crate::error::AppResult;
use crate::{timer, workday};

#[derive(Debug, Clone, PartialEq)]
pub struct TimerInfo {
    pub entry_id: i64,
    pub task_key: String,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BreakInfo {
    pub id: i64,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkdayInfo {
    pub work_date: String,
    pub started_at: DateTime<Utc>,
    pub open_break: Option<BreakInfo>,
    pub last_break_end: Option<DateTime<Utc>>,
    /// Worked time today across all sessions, breaks excluded — same figure as the
    /// workday widget shows.
    pub worked_seconds_today: i64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReminderInputs {
    pub timer: Option<TimerInfo>,
    pub workday: Option<WorkdayInfo>,
    pub last_entry_end: Option<DateTime<Utc>>,
}

/// What was already shown. In memory only: after a restart a still-true reminder may
/// fire once more, which is acceptable.
#[derive(Debug, Default)]
pub struct ReminderMemory {
    /// Running entry id, and how many thresholds it had passed when last notified.
    long_timer: Option<(i64, i64)>,
    /// The start of the idle stretch last notified about.
    idle_since: Option<DateTime<Utc>>,
    /// The break last notified about.
    long_break: Option<i64>,
    /// The workday date last notified about — at most once per day.
    end_workday: Option<String>,
}

pub fn load_inputs(conn: &Connection, now: DateTime<Utc>) -> AppResult<ReminderInputs> {
    let timer = timer::engine::get_running(conn)?.map(|entry| TimerInfo {
        entry_id: entry.id,
        task_key: entry.task_key,
        started_at: entry.started_at,
    });
    let workday = match workday::engine::get_running(conn)? {
        Some(day) => {
            let breaks = work_days_repo::breaks_for_day(conn, day.id)?;
            let date = NaiveDate::parse_from_str(&day.work_date, "%Y-%m-%d")
                .expect("work_date is always YYYY-MM-DD, set by engine::start_workday");
            let (prior_worked, _) = workday::engine::prior_today_seconds(conn, date, day.id, now)?;
            Some(WorkdayInfo {
                worked_seconds_today: prior_worked + workday::engine::worked_seconds(&day, &breaks, now),
                open_break: breaks.iter().find(|b| b.is_running()).map(|b| BreakInfo {
                    id: b.id,
                    started_at: b.started_at,
                }),
                last_break_end: breaks.iter().filter_map(|b| b.ended_at).max(),
                work_date: day.work_date,
                started_at: day.started_at,
            })
        }
        None => None,
    };
    Ok(ReminderInputs {
        timer,
        workday,
        last_entry_end: time_entries_repo::latest_ended_at(conn)?,
    })
}

/// `4h`, `1h 30m`, `45m` — the same shape as the duration inputs in the UI.
pub fn human_duration(seconds: i64) -> String {
    let minutes = seconds.max(0) / 60;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

pub fn due(
    inputs: &ReminderInputs,
    settings: &NotificationSettings,
    memory: &mut ReminderMemory,
    now: DateTime<Utc>,
) -> Vec<Notice> {
    let mut notices = Vec::new();

    // Repeats every time the timer passes another multiple of the threshold. Counting
    // passed thresholds (rather than "notified yet?") also means a timer that's already
    // far past it on app start notifies once, not once per missed threshold.
    match (&inputs.timer, settings.long_timer) {
        (Some(t), true) => {
            let elapsed = (now - t.started_at).num_seconds();
            let passed = elapsed / (settings.long_timer_minutes * 60);
            let already = match memory.long_timer {
                Some((id, n)) if id == t.entry_id => n,
                _ => 0,
            };
            if passed > already {
                notices.push(Notice::new(
                    "Timer still running",
                    format!("{} has been running for {}. Still on it?", t.task_key, human_duration(elapsed)),
                ));
            }
            memory.long_timer = Some((t.entry_id, passed.max(already)));
        }
        _ => memory.long_timer = None,
    }

    if let Some(w) = &inputs.workday {
        if settings.idle_workday && inputs.timer.is_none() && w.open_break.is_none() {
            // Idle since whichever happened last: clocking in, coming back from a
            // break, or stopping the last timer.
            let idle_since = [Some(w.started_at), w.last_break_end, inputs.last_entry_end]
                .into_iter()
                .flatten()
                .max()
                .expect("started_at is always present");
            let idle = (now - idle_since).num_seconds();
            if idle >= settings.idle_workday_minutes * 60 && memory.idle_since != Some(idle_since) {
                notices.push(Notice::new(
                    "No timer running",
                    format!("You're clocked in, but no timer has run for {}.", human_duration(idle)),
                ));
                memory.idle_since = Some(idle_since);
            }
        }

        if let (Some(b), true) = (&w.open_break, settings.long_break) {
            let elapsed = (now - b.started_at).num_seconds();
            if elapsed >= settings.long_break_minutes * 60 && memory.long_break != Some(b.id) {
                notices.push(Notice::new(
                    "Break still running",
                    format!("Your break has lasted {}. Forgot to end it?", human_duration(elapsed)),
                ));
                memory.long_break = Some(b.id);
            }
        }

        if settings.forgot_end_workday
            && w.worked_seconds_today >= settings.end_workday_minutes * 60
            && memory.end_workday.as_deref() != Some(w.work_date.as_str())
        {
            notices.push(Notice::new(
                "Workday still running",
                format!(
                    "You've worked {} today and the workday is still running. Forgot to clock out?",
                    human_duration(w.worked_seconds_today)
                ),
            ));
            memory.end_workday = Some(w.work_date.clone());
        }
    }

    notices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;
    use crate::db::tasks_repo;
    use chrono::{Duration, TimeZone};

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 11, 15, 0, 0).unwrap()
    }

    fn mins(m: i64) -> Duration {
        Duration::minutes(m)
    }

    fn titles(notices: &[Notice]) -> Vec<&str> {
        notices.iter().map(|n| n.title.as_str()).collect()
    }

    fn timer(started_at: DateTime<Utc>) -> Option<TimerInfo> {
        Some(TimerInfo {
            entry_id: 1,
            task_key: "PROJ-1".into(),
            started_at,
        })
    }

    fn workday(started_at: DateTime<Utc>) -> WorkdayInfo {
        WorkdayInfo {
            work_date: "2026-08-11".into(),
            started_at,
            open_break: None,
            last_break_end: None,
            worked_seconds_today: 0,
        }
    }

    #[test]
    fn long_timer_repeats_per_threshold_and_catches_up_once() {
        let settings = NotificationSettings::default(); // 4h
        let mut memory = ReminderMemory::default();
        let started = now() - mins(4 * 60 + 1);
        let inputs = ReminderInputs {
            timer: timer(started),
            ..Default::default()
        };

        let first = due(&inputs, &settings, &mut memory, now());
        assert_eq!(titles(&first), ["Timer still running"]);
        assert!(first[0].body.contains("4h 1m"));
        assert!(due(&inputs, &settings, &mut memory, now() + mins(60)).is_empty());
        assert_eq!(due(&inputs, &settings, &mut memory, now() + mins(4 * 60)).len(), 1);

        // Already 9h in when first seen (e.g. app restart): one notice, not two.
        let mut fresh = ReminderMemory::default();
        let late = ReminderInputs {
            timer: timer(now() - mins(9 * 60)),
            ..Default::default()
        };
        assert_eq!(due(&late, &settings, &mut fresh, now()).len(), 1);
        assert!(due(&late, &settings, &mut fresh, now() + mins(1)).is_empty());
    }

    #[test]
    fn idle_counts_from_the_latest_of_clock_in_break_end_and_timer_stop() {
        let settings = NotificationSettings::default(); // 30m
        let mut memory = ReminderMemory::default();
        let mut inputs = ReminderInputs {
            workday: Some(WorkdayInfo {
                last_break_end: Some(now() - mins(40)),
                ..workday(now() - mins(120))
            }),
            last_entry_end: Some(now() - mins(20)),
            ..Default::default()
        };
        assert!(due(&inputs, &settings, &mut memory, now()).is_empty(), "only 20m idle");

        let notices = due(&inputs, &settings, &mut memory, now() + mins(10));
        assert_eq!(titles(&notices), ["No timer running"]);
        assert!(due(&inputs, &settings, &mut memory, now() + mins(60)).is_empty(), "once per stretch");

        // A new stretch (timer ran and stopped again) can notify again.
        inputs.last_entry_end = Some(now() + mins(70));
        assert_eq!(due(&inputs, &settings, &mut memory, now() + mins(100)).len(), 1);
    }

    #[test]
    fn idle_is_silent_on_a_break_or_with_a_timer_running() {
        let settings = NotificationSettings::default();
        let on_break = ReminderInputs {
            workday: Some(WorkdayInfo {
                open_break: Some(BreakInfo {
                    id: 1,
                    started_at: now() - mins(5),
                }),
                ..workday(now() - mins(120))
            }),
            ..Default::default()
        };
        assert!(due(&on_break, &settings, &mut ReminderMemory::default(), now()).is_empty());

        let with_timer = ReminderInputs {
            timer: timer(now() - mins(5)),
            workday: Some(workday(now() - mins(120))),
            ..Default::default()
        };
        assert!(due(&with_timer, &settings, &mut ReminderMemory::default(), now()).is_empty());
    }

    #[test]
    fn long_break_fires_once_per_break() {
        let settings = NotificationSettings::default(); // 60m
        let mut memory = ReminderMemory::default();
        let inputs = ReminderInputs {
            workday: Some(WorkdayInfo {
                open_break: Some(BreakInfo {
                    id: 7,
                    started_at: now() - mins(61),
                }),
                ..workday(now() - mins(300))
            }),
            ..Default::default()
        };
        assert_eq!(titles(&due(&inputs, &settings, &mut memory, now())), ["Break still running"]);
        assert!(due(&inputs, &settings, &mut memory, now() + mins(60)).is_empty());
    }

    #[test]
    fn end_of_workday_follows_worked_time_once_per_day() {
        let settings = NotificationSettings::default(); // 9h
        let mut memory = ReminderMemory::default();
        let mut inputs = ReminderInputs {
            timer: timer(now() - mins(10)),
            workday: Some(WorkdayInfo {
                worked_seconds_today: 8 * 3600,
                ..workday(now() - mins(9 * 60))
            }),
            ..Default::default()
        };
        assert!(due(&inputs, &settings, &mut memory, now()).is_empty());

        inputs.workday.as_mut().unwrap().worked_seconds_today = 9 * 3600;
        assert_eq!(titles(&due(&inputs, &settings, &mut memory, now())), ["Workday still running"]);
        assert!(due(&inputs, &settings, &mut memory, now()).is_empty());
    }

    #[test]
    fn disabled_reminders_stay_silent() {
        let settings = NotificationSettings {
            long_timer: false,
            idle_workday: false,
            long_break: false,
            forgot_end_workday: false,
            ..NotificationSettings::default()
        };
        let inputs = ReminderInputs {
            timer: timer(now() - mins(10 * 60)),
            workday: Some(WorkdayInfo {
                open_break: Some(BreakInfo {
                    id: 1,
                    started_at: now() - mins(120),
                }),
                worked_seconds_today: 10 * 3600,
                ..workday(now() - mins(12 * 60))
            }),
            ..Default::default()
        };
        assert!(due(&inputs, &settings, &mut ReminderMemory::default(), now()).is_empty());
    }

    #[test]
    fn load_inputs_reads_timer_workday_and_last_entry() {
        let mut conn = open_in_memory().unwrap();
        let task = tasks_repo::upsert_task(&conn, "PROJ-1", "A", now()).unwrap();
        workday::engine::start_workday(&conn, now() - mins(180)).unwrap();
        workday::engine::start_break(&conn, now() - mins(120)).unwrap();
        workday::engine::end_break(&conn, now() - mins(90)).unwrap();
        timer::engine::start(&mut conn, task.id, None, now() - mins(60)).unwrap();
        timer::engine::stop(&conn, now() - mins(30)).unwrap();

        let inputs = load_inputs(&conn, now()).unwrap();
        assert_eq!(inputs.timer, None);
        assert_eq!(inputs.last_entry_end, Some(now() - mins(30)));
        let w = inputs.workday.unwrap();
        assert_eq!(w.last_break_end, Some(now() - mins(90)));
        assert_eq!(w.open_break, None);
        assert_eq!(w.worked_seconds_today, 150 * 60);
    }

    #[test]
    fn human_duration_matches_ui_input_shape() {
        assert_eq!(human_duration(45 * 60), "45m");
        assert_eq!(human_duration(4 * 3600), "4h");
        assert_eq!(human_duration(90 * 60 + 59), "1h 30m");
    }
}
