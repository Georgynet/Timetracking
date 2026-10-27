//! Notification preferences, stored as `notify.*` keys in the `preferences` key/value
//! table (ADR-0026) — every type can be switched off, and the reminders have their own
//! thresholds.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::preferences_repo;
use crate::error::{AppError, AppResult};

const SYNC_RESULT: &str = "notify.sync_result";
const UNSYNCED_ON_END_WORKDAY: &str = "notify.unsynced_on_end_workday";
const LONG_TIMER: &str = "notify.long_timer";
const LONG_TIMER_MINUTES: &str = "notify.long_timer_minutes";
const IDLE_WORKDAY: &str = "notify.idle_workday";
const IDLE_WORKDAY_MINUTES: &str = "notify.idle_workday_minutes";
const LONG_BREAK: &str = "notify.long_break";
const LONG_BREAK_MINUTES: &str = "notify.long_break_minutes";
const FORGOT_END_WORKDAY: &str = "notify.forgot_end_workday";
const END_WORKDAY_MINUTES: &str = "notify.end_workday_minutes";

/// A threshold longer than a day can never fire for a workday-sized session.
const MAX_MINUTES: i64 = 24 * 60;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    /// Result of a sync started from the tray (the window is usually hidden then).
    pub sync_result: bool,
    /// Entries still unsynced when the workday is ended.
    pub unsynced_on_end_workday: bool,
    /// A timer running this long — repeated every time it runs that long again.
    pub long_timer: bool,
    pub long_timer_minutes: i64,
    /// Clocked in, not on a break, and no timer running for this long.
    pub idle_workday: bool,
    pub idle_workday_minutes: i64,
    /// A break running this long.
    pub long_break: bool,
    pub long_break_minutes: i64,
    /// A workday still running once today's worked time reaches this — relative to
    /// how much was worked rather than a clock time, since start times vary by day.
    pub forgot_end_workday: bool,
    pub end_workday_minutes: i64,
}

/// Everything on by default — reminders nobody knows about don't help anyone.
impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            sync_result: true,
            unsynced_on_end_workday: true,
            long_timer: true,
            long_timer_minutes: 4 * 60,
            idle_workday: true,
            idle_workday_minutes: 30,
            long_break: true,
            long_break_minutes: 60,
            forgot_end_workday: true,
            end_workday_minutes: 9 * 60,
        }
    }
}

impl NotificationSettings {
    pub fn validate(&self) -> AppResult<()> {
        for (label, minutes) in [
            ("Long timer reminder", self.long_timer_minutes),
            ("No timer reminder", self.idle_workday_minutes),
            ("Long break reminder", self.long_break_minutes),
            ("End of workday reminder", self.end_workday_minutes),
        ] {
            if !(1..=MAX_MINUTES).contains(&minutes) {
                return Err(AppError::Validation(format!(
                    "{label} must be between 1 minute and 24 hours."
                )));
            }
        }
        Ok(())
    }
}

fn get_bool(conn: &Connection, key: &str, default: bool) -> rusqlite::Result<bool> {
    Ok(preferences_repo::get_i64(conn, key, default as i64)? != 0)
}

/// Missing or malformed values fall back to the defaults, like every other preference.
pub fn load(conn: &Connection) -> rusqlite::Result<NotificationSettings> {
    let d = NotificationSettings::default();
    let minutes = |key, default| -> rusqlite::Result<i64> {
        let value = preferences_repo::get_i64(conn, key, default)?;
        Ok(if (1..=MAX_MINUTES).contains(&value) { value } else { default })
    };
    Ok(NotificationSettings {
        sync_result: get_bool(conn, SYNC_RESULT, d.sync_result)?,
        unsynced_on_end_workday: get_bool(conn, UNSYNCED_ON_END_WORKDAY, d.unsynced_on_end_workday)?,
        long_timer: get_bool(conn, LONG_TIMER, d.long_timer)?,
        long_timer_minutes: minutes(LONG_TIMER_MINUTES, d.long_timer_minutes)?,
        idle_workday: get_bool(conn, IDLE_WORKDAY, d.idle_workday)?,
        idle_workday_minutes: minutes(IDLE_WORKDAY_MINUTES, d.idle_workday_minutes)?,
        long_break: get_bool(conn, LONG_BREAK, d.long_break)?,
        long_break_minutes: minutes(LONG_BREAK_MINUTES, d.long_break_minutes)?,
        forgot_end_workday: get_bool(conn, FORGOT_END_WORKDAY, d.forgot_end_workday)?,
        end_workday_minutes: minutes(END_WORKDAY_MINUTES, d.end_workday_minutes)?,
    })
}

pub fn save(conn: &Connection, s: &NotificationSettings) -> rusqlite::Result<()> {
    preferences_repo::set_i64(conn, SYNC_RESULT, s.sync_result as i64)?;
    preferences_repo::set_i64(conn, UNSYNCED_ON_END_WORKDAY, s.unsynced_on_end_workday as i64)?;
    preferences_repo::set_i64(conn, LONG_TIMER, s.long_timer as i64)?;
    preferences_repo::set_i64(conn, LONG_TIMER_MINUTES, s.long_timer_minutes)?;
    preferences_repo::set_i64(conn, IDLE_WORKDAY, s.idle_workday as i64)?;
    preferences_repo::set_i64(conn, IDLE_WORKDAY_MINUTES, s.idle_workday_minutes)?;
    preferences_repo::set_i64(conn, LONG_BREAK, s.long_break as i64)?;
    preferences_repo::set_i64(conn, LONG_BREAK_MINUTES, s.long_break_minutes)?;
    preferences_repo::set_i64(conn, FORGOT_END_WORKDAY, s.forgot_end_workday as i64)?;
    preferences_repo::set_i64(conn, END_WORKDAY_MINUTES, s.end_workday_minutes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;

    #[test]
    fn defaults_until_saved_then_round_trips() {
        let conn = open_in_memory().unwrap();
        assert_eq!(load(&conn).unwrap(), NotificationSettings::default());

        let custom = NotificationSettings {
            sync_result: false,
            long_timer_minutes: 90,
            end_workday_minutes: 8 * 60,
            ..NotificationSettings::default()
        };
        save(&conn, &custom).unwrap();
        assert_eq!(load(&conn).unwrap(), custom);
    }

    #[test]
    fn malformed_stored_values_fall_back_to_defaults() {
        let conn = open_in_memory().unwrap();
        preferences_repo::set(&conn, LONG_TIMER_MINUTES, "0").unwrap();
        preferences_repo::set(&conn, END_WORKDAY_MINUTES, "late").unwrap();
        let loaded = load(&conn).unwrap();
        assert_eq!(loaded.long_timer_minutes, 240);
        assert_eq!(loaded.end_workday_minutes, 540);
    }

    #[test]
    fn validation_rejects_out_of_range_thresholds() {
        assert!(NotificationSettings::default().validate().is_ok());
        let zero = NotificationSettings {
            idle_workday_minutes: 0,
            ..NotificationSettings::default()
        };
        assert!(zero.validate().is_err());
        let too_long = NotificationSettings {
            end_workday_minutes: 25 * 60,
            ..NotificationSettings::default()
        };
        assert!(too_long.validate().is_err());
    }
}
