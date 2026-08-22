pub mod entries;
pub mod preferences;
pub mod setup;
pub mod stats;
pub mod sync;
pub mod tasks;
pub mod timer;
pub mod workday;

use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub fn is_tray_available(state: State<'_, AppState>) -> bool {
    state.is_tray_available()
}
