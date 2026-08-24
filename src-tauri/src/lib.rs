mod commands;
mod db;
mod error;
// `pub` so the wiremock-based integration tests in `tests/jira_client_tests.rs` can
// exercise `ReqwestJiraClient` directly.
pub mod jira;
mod logging;
mod secrets;
mod state;
mod stats;
mod sync;
mod timer;
mod workday;

use std::sync::Arc;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, Manager};

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("app data dir should be resolvable");
            std::fs::create_dir_all(&app_dir).expect("could not create app data directory");
            // `cfg!(debug_assertions)` is true for `tauri dev` (debug builds) and false
            // for `tauri build` (release builds) — using it here keeps dev and prod on
            // separate SQLite files (and, via `keyring_store`, separate keychain
            // entries) without needing a dedicated env var or build profile.
            let db_filename = if cfg!(debug_assertions) {
                "timetracking-dev.sqlite3"
            } else {
                "timetracking.sqlite3"
            };
            let db_path = app_dir.join(db_filename);
            let conn = db::connection::open_app_db(&db_path).expect("failed to open/migrate database");

            let state = AppState::new(conn);
            restore_jira_client(&state);
            app.manage(state);

            // Built programmatically (not declared in tauri.conf.json) so failure can
            // be caught here and the app falls back to a window-only mode instead of
            // crashing — some Linux desktop environments (stock GNOME without an
            // AppIndicator extension) have no tray support at all.
            let tray_available = build_tray(app).is_ok();
            if !tray_available {
                tracing::warn!("system tray unavailable; falling back to window-only mode");
            }
            app.state::<AppState>().set_tray_available(tray_available);

            // Debug builds use a separate db/keychain (see above) but look identical
            // otherwise — tag the window title so it's obvious at a glance which one
            // is running, especially with both open side by side.
            if cfg!(debug_assertions) {
                if let Some(window) = app.get_webview_window("main") {
                    let dev_title = format!("{} - DEV", window.title().unwrap_or_default());
                    let _ = window.set_title(&dev_title);
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::is_tray_available,
            commands::setup::get_settings,
            commands::setup::save_jira_settings,
            commands::setup::test_jira_connection,
            commands::setup::clear_jira_settings,
            commands::tasks::refresh_my_tasks,
            commands::tasks::list_my_tasks,
            commands::tasks::list_favorite_tasks,
            commands::tasks::search_jira_issues,
            commands::tasks::add_favorite_by_key,
            commands::tasks::resolve_task_by_key,
            commands::tasks::remove_favorite,
            commands::timer::get_active_timer,
            commands::timer::start_timer,
            commands::timer::stop_timer,
            commands::entries::create_manual_entry,
            commands::entries::update_time_entry,
            commands::entries::list_time_entries,
            commands::entries::delete_draft_entry,
            commands::sync::sync_all,
            commands::sync::list_unsynced_count,
            commands::workday::get_active_workday,
            commands::workday::start_workday,
            commands::workday::end_workday,
            commands::workday::start_break,
            commands::workday::end_break,
            commands::workday::update_break,
            commands::workday::get_daily_summary,
            commands::workday::get_week_summary,
            commands::workday::get_month_summary,
            commands::stats::get_ticket_stats,
            commands::stats::get_interval_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Rebuilds the in-memory Jira client from saved settings + the keychain token, if
/// both are present, so the app doesn't need to touch the keychain again on every API
/// call after startup.
fn restore_jira_client(state: &AppState) {
    let settings = {
        let conn = state.db.lock().unwrap();
        db::settings_repo::get_settings(&conn)
    };
    let Ok(Some(settings)) = settings else { return };
    let Ok(Some(token)) = secrets::keyring_store::load_token() else { return };
    if let (Some(base_url), Some(email)) = (settings.jira_base_url, settings.jira_email) {
        state.set_jira_client(Arc::new(jira::reqwest_client::ReqwestJiraClient::new(
            base_url, email, token,
        )));
    }
}

fn build_tray(app: &App) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&quit])?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?;
    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .on_menu_event(|app, event| {
            if event.id() == "quit" {
                app.exit(0);
            }
        })
        .build(app)?;
    Ok(())
}
