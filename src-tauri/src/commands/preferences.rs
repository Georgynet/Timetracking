use serde::Serialize;
use tauri::State;

use crate::db::preferences_repo;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

const MY_TASKS_ROWS: &str = "ui.my_tasks_rows";
const FAVORITES_ROWS: &str = "ui.favorites_rows";
const CURRENT_SPRINT_DEFAULT: &str = "ui.current_sprint_default";
const TICKET_ORDER: &str = "ui.ticket_order";
const THEME: &str = "ui.theme";

/// How many rows each task panel shows before it starts scrolling. Defaults match the
/// heights the panels had before this was configurable.
const DEFAULT_MY_TASKS_ROWS: i64 = 5;
const DEFAULT_FAVORITES_ROWS: i64 = 4;
/// On by default — the sprint is what's being worked on nearly every time (ADR-0024).
const DEFAULT_CURRENT_SPRINT: bool = true;

/// Ordering for the ticket pickers. "recent" puts what you last tracked at the top —
/// the default, since the next thing you track is usually something you tracked
/// lately; "key" is the plain alphabetical order by ticket key.
const TICKET_ORDER_VALUES: [&str; 2] = ["recent", "key"];
const DEFAULT_TICKET_ORDER: &str = "recent";

/// Light, dark, or whatever the OS is set to. Stored as the three-way choice, not the
/// resolved scheme — the frontend resolves "system" and re-resolves it live when the
/// OS flips (see ADR-0027).
const THEME_VALUES: [&str; 3] = ["system", "light", "dark"];
const DEFAULT_THEME: &str = "system";

/// A panel taller than this pushes everything below it (the timer, History) off the
/// screen, which is a worse problem than scrolling inside the panel.
const MAX_ROWS: i64 = 25;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesDto {
    pub my_tasks_rows: i64,
    pub favorites_rows: i64,
    /// Whether My Tasks starts filtered to the current sprint on launch.
    pub current_sprint_default: bool,
    /// How the ticket pickers are ordered: `"recent"` or `"key"`.
    pub ticket_order: String,
    /// `"system"`, `"light"` or `"dark"`.
    pub theme: String,
}

fn get_preferences_impl(state: &AppState) -> AppResult<PreferencesDto> {
    let conn = state.db.lock().unwrap();
    Ok(PreferencesDto {
        my_tasks_rows: preferences_repo::get_i64(&conn, MY_TASKS_ROWS, DEFAULT_MY_TASKS_ROWS)?,
        favorites_rows: preferences_repo::get_i64(&conn, FAVORITES_ROWS, DEFAULT_FAVORITES_ROWS)?,
        current_sprint_default: preferences_repo::get_i64(
            &conn,
            CURRENT_SPRINT_DEFAULT,
            DEFAULT_CURRENT_SPRINT as i64,
        )? != 0,
        ticket_order: preferences_repo::get(&conn, TICKET_ORDER)?
            .filter(|v| TICKET_ORDER_VALUES.contains(&v.as_str()))
            .unwrap_or_else(|| DEFAULT_TICKET_ORDER.to_string()),
        theme: preferences_repo::get(&conn, THEME)?
            .filter(|v| THEME_VALUES.contains(&v.as_str()))
            .unwrap_or_else(|| DEFAULT_THEME.to_string()),
    })
}

#[tauri::command]
pub fn get_preferences(state: State<'_, AppState>) -> AppResult<PreferencesDto> {
    get_preferences_impl(&state)
}

fn check_rows(label: &str, rows: i64) -> AppResult<()> {
    if !(1..=MAX_ROWS).contains(&rows) {
        return Err(AppError::Validation(format!(
            "{label} must be between 1 and {MAX_ROWS}."
        )));
    }
    Ok(())
}

fn save_preferences_impl(
    state: &AppState,
    my_tasks_rows: i64,
    favorites_rows: i64,
    current_sprint_default: bool,
    ticket_order: String,
    theme: String,
) -> AppResult<PreferencesDto> {
    check_rows("My Tasks rows", my_tasks_rows)?;
    check_rows("Favorites rows", favorites_rows)?;
    if !TICKET_ORDER_VALUES.contains(&ticket_order.as_str()) {
        return Err(AppError::Validation(format!(
            "Unknown ticket order: {ticket_order}."
        )));
    }
    if !THEME_VALUES.contains(&theme.as_str()) {
        return Err(AppError::Validation(format!("Unknown theme: {theme}.")));
    }
    {
        let conn = state.db.lock().unwrap();
        preferences_repo::set_i64(&conn, MY_TASKS_ROWS, my_tasks_rows)?;
        preferences_repo::set_i64(&conn, FAVORITES_ROWS, favorites_rows)?;
        preferences_repo::set_i64(&conn, CURRENT_SPRINT_DEFAULT, current_sprint_default as i64)?;
        preferences_repo::set(&conn, TICKET_ORDER, &ticket_order)?;
        preferences_repo::set(&conn, THEME, &theme)?;
    }
    get_preferences_impl(state)
}

#[tauri::command]
pub fn save_preferences(
    state: State<'_, AppState>,
    my_tasks_rows: i64,
    favorites_rows: i64,
    current_sprint_default: bool,
    ticket_order: String,
    theme: String,
) -> AppResult<PreferencesDto> {
    save_preferences_impl(
        &state,
        my_tasks_rows,
        favorites_rows,
        current_sprint_default,
        ticket_order,
        theme,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;

    fn setup() -> AppState {
        AppState::new(open_in_memory().unwrap())
    }

    #[test]
    fn defaults_apply_until_something_is_saved() {
        let state = setup();
        let prefs = get_preferences_impl(&state).unwrap();
        assert_eq!(prefs.my_tasks_rows, DEFAULT_MY_TASKS_ROWS);
        assert_eq!(prefs.favorites_rows, DEFAULT_FAVORITES_ROWS);
        assert!(prefs.current_sprint_default, "the sprint filter starts on");
        assert_eq!(prefs.ticket_order, DEFAULT_TICKET_ORDER);
        assert_eq!(prefs.theme, DEFAULT_THEME);
    }

    #[test]
    fn saved_row_counts_round_trip() {
        let state = setup();
        let saved = save_preferences_impl(&state, 10, 12, false, "key".into(), "dark".into()).unwrap();
        assert_eq!((saved.my_tasks_rows, saved.favorites_rows), (10, 12));
        assert!(!saved.current_sprint_default);
        assert_eq!(saved.ticket_order, "key");
        assert_eq!(saved.theme, "dark");
        let reloaded = get_preferences_impl(&state).unwrap();
        assert_eq!((reloaded.my_tasks_rows, reloaded.favorites_rows), (10, 12));
        assert!(!reloaded.current_sprint_default, "the toggle's default must persist");
        assert_eq!(reloaded.ticket_order, "key");
        assert_eq!(reloaded.theme, "dark", "an explicit theme must persist");
    }

    #[test]
    fn an_unreadable_stored_ordering_falls_back_to_the_default() {
        let state = setup();
        {
            let conn = state.db.lock().unwrap();
            preferences_repo::set(&conn, TICKET_ORDER, "nonsense").unwrap();
            preferences_repo::set(&conn, THEME, "chartreuse").unwrap();
        }
        let prefs = get_preferences_impl(&state).unwrap();
        assert_eq!(prefs.ticket_order, DEFAULT_TICKET_ORDER);
        assert_eq!(prefs.theme, DEFAULT_THEME);
    }

    #[test]
    fn out_of_range_row_counts_are_rejected_and_change_nothing() {
        let state = setup();
        save_preferences_impl(&state, 6, 6, true, "recent".into(), "light".into()).unwrap();

        assert!(save_preferences_impl(&state, 0, 6, true, "recent".into(), "light".into()).is_err());
        assert!(
            save_preferences_impl(&state, 6, MAX_ROWS + 1, true, "recent".into(), "light".into())
                .is_err()
        );
        assert!(
            save_preferences_impl(&state, 6, 6, true, "sideways".into(), "light".into()).is_err(),
            "an unknown ordering must be rejected, not stored"
        );
        assert!(
            save_preferences_impl(&state, 6, 6, true, "recent".into(), "neon".into()).is_err(),
            "an unknown theme must be rejected, not stored"
        );

        let prefs = get_preferences_impl(&state).unwrap();
        assert_eq!((prefs.my_tasks_rows, prefs.favorites_rows), (6, 6));
        assert_eq!(prefs.theme, "light", "the rejected saves must not have changed it");
    }
}
