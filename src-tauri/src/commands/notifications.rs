use tauri::State;

use crate::error::AppResult;
use crate::notifications::{self, settings, settings::NotificationSettings};
use crate::state::AppState;

/// Lets the frontend hide the Notifications settings where they'd have no effect.
#[tauri::command]
pub fn notifications_supported() -> bool {
    notifications::supported()
}

#[tauri::command]
pub fn get_notification_settings(state: State<'_, AppState>) -> AppResult<NotificationSettings> {
    let conn = state.db.lock().unwrap();
    Ok(settings::load(&conn)?)
}

fn save_notification_settings_impl(
    state: &AppState,
    settings: NotificationSettings,
) -> AppResult<NotificationSettings> {
    settings.validate()?;
    let conn = state.db.lock().unwrap();
    settings::save(&conn, &settings)?;
    Ok(settings::load(&conn)?)
}

#[tauri::command]
pub fn save_notification_settings(
    state: State<'_, AppState>,
    settings: NotificationSettings,
) -> AppResult<NotificationSettings> {
    save_notification_settings_impl(&state, settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;

    #[test]
    fn invalid_settings_are_rejected_without_saving() {
        let state = AppState::new(open_in_memory().unwrap());
        let invalid = NotificationSettings {
            long_break_minutes: 0,
            ..NotificationSettings::default()
        };
        assert!(save_notification_settings_impl(&state, invalid).is_err());
        let conn = state.db.lock().unwrap();
        assert_eq!(settings::load(&conn).unwrap(), NotificationSettings::default());
    }
}
