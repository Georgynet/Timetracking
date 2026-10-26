//! The system tray: a status + quick-actions menu (timer, workday/breaks, sync).
//!
//! The menu is rebuilt from the database whenever its *structure* can change — after a
//! tray action, and after the UI commands that start/stop the timer, workday or a
//! break, or change favorites (they call `refresh`). Between rebuilds a background
//! ticker only updates the texts of the status/sync items, so elapsed times stay
//! current without swapping the menu out from under a user who has it open.
//!
//! Actions taken from the tray emit `STATE_CHANGED_EVENT` so the frontend re-reads
//! state from the backend, the same way it does after its own actions.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Local, NaiveDate, Utc};
use rusqlite::Connection;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Emitter, Manager, Wry};

use crate::commands::sync::sync_all_impl;
use crate::db::models::Task;
use crate::db::{tasks_repo, time_entries_repo, work_days_repo};
use crate::error::AppResult;
use crate::state::AppState;
use crate::{timer, workday};

const TRAY_ID: &str = "main";
pub const STATE_CHANGED_EVENT: &str = "tray-state-changed";
pub const SYNC_FINISHED_EVENT: &str = "tray-sync-finished";
const TICK_INTERVAL: Duration = Duration::from_secs(30);
const MAX_FAVORITES: usize = 15;
const MAX_RECENT: usize = 5;
const SUMMARY_MAX_CHARS: usize = 40;

#[derive(Debug, Clone, PartialEq)]
pub struct TaskRef {
    pub id: i64,
    pub key: String,
    pub summary: String,
}

impl From<&Task> for TaskRef {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id,
            key: task.jira_key.clone(),
            summary: task.summary.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunningTimer {
    pub task: TaskRef,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum WorkdayStatus {
    #[default]
    NotStarted,
    /// No workday is running, but some time was already clocked today.
    Ended { worked_seconds: i64 },
    Working { since: DateTime<Utc>, worked_seconds: i64 },
    OnBreak { since: DateTime<Utc>, worked_seconds: i64 },
}

/// Everything the tray menu shows, read from the database in one go.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrayModel {
    pub running: Option<RunningTimer>,
    /// The most recently tracked ticket, offered as "Resume" while no timer runs.
    pub resume: Option<TaskRef>,
    /// Favorites, most recently tracked first (like the UI's "recent" ticket order),
    /// excluding the ticket whose timer is running.
    pub favorites: Vec<TaskRef>,
    /// Recently tracked tickets that aren't favorites, excluding the running one.
    pub recent: Vec<TaskRef>,
    pub workday: WorkdayStatus,
    pub unsynced: i64,
}

pub fn snapshot(conn: &Connection, now: DateTime<Utc>) -> AppResult<TrayModel> {
    let running = timer::engine::get_running(conn)?.map(|entry| RunningTimer {
        task: TaskRef {
            id: entry.task_id,
            key: entry.task_key,
            summary: entry.task_summary,
        },
        started_at: entry.started_at,
    });
    let running_id = running.as_ref().map(|r| r.task.id);

    let recently_tracked = tasks_repo::list_recently_tracked(conn, MAX_FAVORITES + MAX_RECENT + 1)?;
    let resume = match running {
        Some(_) => None,
        None => recently_tracked.first().map(TaskRef::from),
    };
    let recent = recently_tracked
        .iter()
        .filter(|t| !t.is_favorite && Some(t.id) != running_id)
        .take(MAX_RECENT)
        .map(TaskRef::from)
        .collect();

    let mut favorites = tasks_repo::list_favorite_tasks(conn)?;
    // Same order as the UI's "recent" ticket order (`src/lib/tasks.ts`): tracked
    // tickets newest first, never-tracked ones after them by key. The repo already
    // returns them by key, and the sort is stable.
    favorites.sort_by(|a, b| match (a.last_tracked_at, b.last_tracked_at) {
        (Some(a), Some(b)) => b.cmp(&a),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    let favorites = favorites
        .iter()
        .filter(|t| Some(t.id) != running_id)
        .take(MAX_FAVORITES)
        .map(TaskRef::from)
        .collect();

    Ok(TrayModel {
        running,
        resume,
        favorites,
        recent,
        workday: workday_status(conn, now)?,
        unsynced: time_entries_repo::count_unsynced(conn)?,
    })
}

/// Worked time mirrors the UI's workday widget: the running session plus whatever
/// earlier sessions today already banked (`engine::prior_today_seconds`).
fn workday_status(conn: &Connection, now: DateTime<Utc>) -> AppResult<WorkdayStatus> {
    let Some(day) = workday::engine::get_running(conn)? else {
        let today = workday::engine::local_date(now);
        let worked_seconds = workday::engine::daily_summary(conn, today, now)?.worked_seconds;
        return Ok(if worked_seconds > 0 {
            WorkdayStatus::Ended { worked_seconds }
        } else {
            WorkdayStatus::NotStarted
        });
    };
    let breaks = work_days_repo::breaks_for_day(conn, day.id)?;
    let date = NaiveDate::parse_from_str(&day.work_date, "%Y-%m-%d")
        .expect("work_date is always YYYY-MM-DD, set by engine::start_workday");
    let (prior_worked, _) = workday::engine::prior_today_seconds(conn, date, day.id, now)?;
    let worked_seconds = prior_worked + workday::engine::worked_seconds(&day, &breaks, now);
    Ok(match breaks.iter().find(|b| b.is_running()) {
        Some(open_break) => WorkdayStatus::OnBreak {
            since: open_break.started_at,
            worked_seconds,
        },
        None => WorkdayStatus::Working {
            since: day.started_at,
            worked_seconds,
        },
    })
}

/// A menu item click, parsed from its id. Keeping ids and their meaning in one place
/// means building the menu and handling its events can't drift apart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayAction {
    StartTimer(i64),
    StopTimer,
    StartWorkday,
    EndWorkday,
    StartBreak,
    EndBreak,
    Sync,
    OpenWindow,
    Quit,
}

impl TrayAction {
    pub fn id(self) -> String {
        match self {
            Self::StartTimer(task_id) => format!("start:{task_id}"),
            Self::StopTimer => "stop".into(),
            Self::StartWorkday => "workday:start".into(),
            Self::EndWorkday => "workday:end".into(),
            Self::StartBreak => "break:start".into(),
            Self::EndBreak => "break:end".into(),
            Self::Sync => "sync".into(),
            Self::OpenWindow => "open".into(),
            Self::Quit => "quit".into(),
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Some(match id {
            "stop" => Self::StopTimer,
            "workday:start" => Self::StartWorkday,
            "workday:end" => Self::EndWorkday,
            "break:start" => Self::StartBreak,
            "break:end" => Self::EndBreak,
            "sync" => Self::Sync,
            "open" => Self::OpenWindow,
            "quit" => Self::Quit,
            _ => Self::StartTimer(id.strip_prefix("start:")?.parse().ok()?),
        })
    }
}

/// `h:mm`, the same compact form for timer elapsed time and worked time.
pub fn format_hm(seconds: i64) -> String {
    let seconds = seconds.max(0);
    format!("{}:{:02}", seconds / 3600, (seconds % 3600) / 60)
}

fn local_hm(t: DateTime<Utc>) -> String {
    t.with_timezone(&Local).format("%H:%M").to_string()
}

fn truncate(s: &str) -> String {
    if s.chars().count() <= SUMMARY_MAX_CHARS {
        return s.to_string();
    }
    let cut: String = s.chars().take(SUMMARY_MAX_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

/// Menu labels treat `&` as a mnemonic marker (macOS strips it, GTK turns it into an
/// underline), so a literal one — common enough in Jira summaries — must be doubled.
fn escape(label: &str) -> String {
    label.replace('&', "&&")
}

pub fn task_label(task: &TaskRef) -> String {
    format!("{}  {}", task.key, truncate(&task.summary))
}

pub fn timer_label(model: &TrayModel, now: DateTime<Utc>) -> String {
    match &model.running {
        Some(r) => format!(
            "● {} · {} — {}",
            r.task.key,
            format_hm((now - r.started_at).num_seconds()),
            truncate(&r.task.summary)
        ),
        None => "No timer running".into(),
    }
}

pub fn workday_label(status: &WorkdayStatus) -> String {
    match status {
        WorkdayStatus::NotStarted => "Workday not started".into(),
        WorkdayStatus::Ended { worked_seconds } => {
            format!("Workday ended · {} worked today", format_hm(*worked_seconds))
        }
        WorkdayStatus::Working { since, worked_seconds } => {
            format!("Working since {} · {} worked", local_hm(*since), format_hm(*worked_seconds))
        }
        WorkdayStatus::OnBreak { since, worked_seconds } => {
            format!("On break since {} · {} worked", local_hm(*since), format_hm(*worked_seconds))
        }
    }
}

pub fn sync_label(unsynced: i64, syncing: bool) -> String {
    if syncing {
        "Syncing…".into()
    } else if unsynced > 0 {
        format!("Sync to Jira ({unsynced} pending)")
    } else {
        "Sync to Jira (all synced)".into()
    }
}

/// Managed state, present only when the tray was built successfully.
#[derive(Default)]
pub struct TrayState {
    handles: Mutex<Option<TextHandles>>,
    syncing: AtomicBool,
}

/// The items whose texts the ticker updates in place between rebuilds.
#[derive(Clone)]
struct TextHandles {
    timer_status: MenuItem<Wry>,
    workday_status: MenuItem<Wry>,
    sync: MenuItem<Wry>,
}

/// Built programmatically (not declared in tauri.conf.json) so failure can be caught
/// by the caller and the app falls back to a window-only mode instead of crashing.
pub fn build(app: &App) -> tauri::Result<()> {
    let handle = app.handle();
    let tray_state = TrayState::default();
    let (menu, handles) = build_menu(handle, &tray_state)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_icon(app)?)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Time Tracker")
        .menu(&menu)
        .on_menu_event(on_menu_event)
        .build(app)?;
    *tray_state.handles.lock().unwrap() = Some(handles);
    app.manage(tray_state);
    spawn_ticker(handle.clone());
    Ok(())
}

/// The macOS menu bar expects a monochrome "template" image that the OS recolors for light
/// and dark menu bars; a full-color icon looks out of place there. Linux panels have no
/// such convention (and a black template would vanish on a dark panel), so they get the
/// regular app icon.
#[cfg(target_os = "macos")]
fn tray_icon(_app: &App) -> tauri::Result<tauri::image::Image<'static>> {
    Ok(tauri::include_image!("icons/tray-template.png"))
}

#[cfg(not(target_os = "macos"))]
fn tray_icon(app: &App) -> tauri::Result<tauri::image::Image<'static>> {
    app.default_window_icon()
        .cloned()
        .map(|icon| icon.to_owned())
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))
}

/// Rebuilds the whole menu from current state. A no-op when the tray isn't available.
/// Callers must not hold the DB lock (this reads from the database).
pub fn refresh(app: &AppHandle) {
    let (Some(tray_state), Some(tray)) = (app.try_state::<TrayState>(), app.tray_by_id(TRAY_ID)) else {
        return;
    };
    match build_menu(app, &tray_state) {
        Ok((menu, handles)) => {
            if let Err(err) = tray.set_menu(Some(menu)) {
                tracing::warn!("failed to replace tray menu: {err}");
            }
            *tray_state.handles.lock().unwrap() = Some(handles);
        }
        Err(err) => tracing::warn!("failed to rebuild tray menu: {err}"),
    }
}

/// Updates only the status and sync texts, leaving the menu structure alone.
/// Callers must not hold the DB lock (this reads from the database).
pub fn update_texts(app: &AppHandle) {
    let Some(tray_state) = app.try_state::<TrayState>() else { return };
    // Cloned out so the lock isn't held while the menu calls dispatch to the main
    // thread — which might itself be waiting on this lock inside `refresh`.
    let Some(handles) = tray_state.handles.lock().unwrap().clone() else { return };
    let (model, jira_configured) = load_model(app);
    let now = Utc::now();
    let syncing = tray_state.syncing.load(Ordering::SeqCst);
    let results = [
        handles.timer_status.set_text(escape(&timer_label(&model, now))),
        handles.workday_status.set_text(escape(&workday_label(&model.workday))),
        handles.sync.set_text(sync_label(model.unsynced, syncing)),
        handles.sync.set_enabled(sync_enabled(&model, jira_configured, syncing)),
    ];
    for err in results.into_iter().filter_map(Result::err) {
        tracing::warn!("failed to update tray menu text: {err}");
    }
}

fn spawn_ticker(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(TICK_INTERVAL);
        update_texts(&app);
    });
}

/// Falls back to an empty model (and logs) on a DB error, so the tray still offers
/// "Open" and "Quit" rather than disappearing.
fn load_model(app: &AppHandle) -> (TrayModel, bool) {
    let state = app.state::<AppState>();
    let jira_configured = state.get_jira_client().is_some();
    let model = {
        let conn = state.db.lock().unwrap();
        snapshot(&conn, Utc::now())
    };
    let model = model.unwrap_or_else(|err| {
        tracing::warn!("failed to read tray state: {err}");
        TrayModel::default()
    });
    (model, jira_configured)
}

fn sync_enabled(model: &TrayModel, jira_configured: bool, syncing: bool) -> bool {
    jira_configured && model.unsynced > 0 && !syncing
}

fn build_menu(app: &AppHandle, tray_state: &TrayState) -> tauri::Result<(Menu<Wry>, TextHandles)> {
    let (model, jira_configured) = load_model(app);
    let now = Utc::now();
    let syncing = tray_state.syncing.load(Ordering::SeqCst);

    let item = |action: TrayAction, label: &str| {
        MenuItem::with_id(app, action.id(), escape(label), true, None::<&str>)
    };
    let disabled = |label: &str| MenuItem::new(app, escape(label), false, None::<&str>);
    let separator = || PredefinedMenuItem::separator(app);

    let menu = Menu::new(app)?;
    let timer_status = disabled(&timer_label(&model, now))?;
    let workday_status = disabled(&workday_label(&model.workday))?;
    menu.append(&timer_status)?;
    menu.append(&workday_status)?;
    menu.append(&separator()?)?;

    if model.running.is_some() {
        menu.append(&item(TrayAction::StopTimer, "Stop timer")?)?;
    } else if let Some(task) = &model.resume {
        menu.append(&item(TrayAction::StartTimer(task.id), &format!("Resume {}", task.key))?)?;
    }
    let start_label = if model.running.is_some() { "Switch to" } else { "Start timer" };
    let start = Submenu::new(app, start_label, true)?;
    for task in &model.favorites {
        start.append(&item(TrayAction::StartTimer(task.id), &task_label(task))?)?;
    }
    if !model.recent.is_empty() {
        if !model.favorites.is_empty() {
            start.append(&separator()?)?;
        }
        start.append(&disabled("Recent")?)?;
        for task in &model.recent {
            start.append(&item(TrayAction::StartTimer(task.id), &task_label(task))?)?;
        }
    }
    if model.favorites.is_empty() && model.recent.is_empty() {
        start.append(&disabled("No favorites yet")?)?;
    }
    menu.append(&start)?;
    menu.append(&separator()?)?;

    match model.workday {
        WorkdayStatus::NotStarted | WorkdayStatus::Ended { .. } => {
            menu.append(&item(TrayAction::StartWorkday, "Start workday")?)?;
        }
        WorkdayStatus::Working { .. } => {
            menu.append(&item(TrayAction::StartBreak, "Start break")?)?;
            menu.append(&item(TrayAction::EndWorkday, "End workday")?)?;
        }
        WorkdayStatus::OnBreak { .. } => {
            menu.append(&item(TrayAction::EndBreak, "End break")?)?;
            menu.append(&item(TrayAction::EndWorkday, "End workday")?)?;
        }
    }
    menu.append(&separator()?)?;

    let sync = MenuItem::with_id(
        app,
        TrayAction::Sync.id(),
        sync_label(model.unsynced, syncing),
        sync_enabled(&model, jira_configured, syncing),
        None::<&str>,
    )?;
    menu.append(&sync)?;
    menu.append(&item(TrayAction::OpenWindow, "Open Time Tracker")?)?;
    menu.append(&separator()?)?;
    menu.append(&item(TrayAction::Quit, "Quit")?)?;

    Ok((
        menu,
        TextHandles {
            timer_status,
            workday_status,
            sync,
        },
    ))
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let Some(action) = TrayAction::parse(event.id().as_ref()) else { return };
    match action {
        TrayAction::Quit => app.exit(0),
        TrayAction::OpenWindow => show_main_window(app),
        TrayAction::Sync => start_sync(app),
        _ => {
            if let Err(err) = apply(app, action) {
                tracing::warn!("tray action {action:?} failed: {err}");
            }
            refresh(app);
            let _ = app.emit(STATE_CHANGED_EVENT, ());
        }
    }
}

/// Runs a state-changing action through the same engines the UI commands use, so the
/// tray gets the same rules (single running timer, break/workday checks) for free.
fn apply(app: &AppHandle, action: TrayAction) -> AppResult<()> {
    let state = app.state::<AppState>();
    let mut conn = state.db.lock().unwrap();
    let now = Utc::now();
    match action {
        TrayAction::StartTimer(task_id) => {
            timer::engine::start(&mut conn, task_id, None, now)?;
        }
        TrayAction::StopTimer => {
            timer::engine::stop(&conn, now)?;
        }
        TrayAction::StartWorkday => {
            workday::engine::start_workday(&conn, now)?;
        }
        TrayAction::EndWorkday => {
            workday::engine::end_workday(&mut conn, now)?;
        }
        TrayAction::StartBreak => {
            workday::engine::start_break(&conn, now)?;
        }
        TrayAction::EndBreak => {
            workday::engine::end_break(&conn, now)?;
        }
        TrayAction::Sync | TrayAction::OpenWindow | TrayAction::Quit => {}
    }
    Ok(())
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Runs in the background so the menu isn't blocked on the network. The item reads
/// "Syncing…" (disabled) meanwhile; the report goes to the frontend, which shows it the
/// same way as a sync started from the UI.
fn start_sync(app: &AppHandle) {
    let tray_state = app.state::<TrayState>();
    if tray_state.syncing.swap(true, Ordering::SeqCst) {
        return;
    }
    update_texts(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = sync_all_impl(&app.state::<AppState>()).await;
        app.state::<TrayState>().syncing.store(false, Ordering::SeqCst);
        match result {
            Ok(report) => {
                let _ = app.emit(SYNC_FINISHED_EVENT, report);
            }
            Err(err) => tracing::warn!("tray sync failed: {err}"),
        }
        update_texts(&app);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 11, 12, 0, 0).unwrap()
    }

    fn track(conn: &Connection, task_id: i64, at: DateTime<Utc>) {
        time_entries_repo::insert_manual(conn, task_id, at, at, 60, None).unwrap();
    }

    #[test]
    fn empty_database_gives_an_empty_model() {
        let conn = open_in_memory().unwrap();
        assert_eq!(snapshot(&conn, now()).unwrap(), TrayModel::default());
    }

    #[test]
    fn running_timer_is_excluded_from_start_lists_and_hides_resume() {
        let mut conn = open_in_memory().unwrap();
        let a = tasks_repo::upsert_favorite_task(&conn, "PROJ-1", "A", now()).unwrap();
        let b = tasks_repo::upsert_favorite_task(&conn, "PROJ-2", "B", now()).unwrap();
        timer::engine::start(&mut conn, a.id, None, now()).unwrap();

        let model = snapshot(&conn, now()).unwrap();
        assert_eq!(model.running.unwrap().task.key, "PROJ-1");
        assert_eq!(model.resume, None);
        assert_eq!(model.favorites, vec![TaskRef::from(&b)]);
    }

    #[test]
    fn resume_offers_the_last_tracked_ticket_when_idle() {
        let conn = open_in_memory().unwrap();
        let a = tasks_repo::upsert_task(&conn, "PROJ-1", "A", now()).unwrap();
        let b = tasks_repo::upsert_task(&conn, "PROJ-2", "B", now()).unwrap();
        track(&conn, b.id, now() - chrono::Duration::hours(2));
        track(&conn, a.id, now() - chrono::Duration::hours(1));

        let model = snapshot(&conn, now()).unwrap();
        assert_eq!(model.resume.unwrap().key, "PROJ-1");
    }

    #[test]
    fn favorites_are_recent_first_and_recent_excludes_favorites() {
        let conn = open_in_memory().unwrap();
        tasks_repo::upsert_favorite_task(&conn, "AAA-1", "Never", now()).unwrap();
        let old = tasks_repo::upsert_favorite_task(&conn, "PROJ-1", "Old", now()).unwrap();
        let new = tasks_repo::upsert_favorite_task(&conn, "PROJ-2", "New", now()).unwrap();
        let other = tasks_repo::upsert_task(&conn, "OTHER-1", "Other", now()).unwrap();
        track(&conn, old.id, now() - chrono::Duration::hours(3));
        track(&conn, new.id, now() - chrono::Duration::hours(2));
        track(&conn, other.id, now() - chrono::Duration::hours(1));

        let model = snapshot(&conn, now()).unwrap();
        let fav_keys: Vec<_> = model.favorites.iter().map(|t| t.key.as_str()).collect();
        assert_eq!(fav_keys, ["PROJ-2", "PROJ-1", "AAA-1"]);
        assert_eq!(model.recent, vec![TaskRef::from(&other)]);
    }

    #[test]
    fn workday_status_tracks_breaks_and_clock_out() {
        let mut conn = open_in_memory().unwrap();
        let start = now() - chrono::Duration::hours(2);
        workday::engine::start_workday(&conn, start).unwrap();
        assert!(matches!(
            snapshot(&conn, now()).unwrap().workday,
            WorkdayStatus::Working { since, worked_seconds: 7200 } if since == start
        ));

        let break_start = now() - chrono::Duration::minutes(30);
        workday::engine::start_break(&conn, break_start).unwrap();
        assert!(matches!(
            snapshot(&conn, now()).unwrap().workday,
            WorkdayStatus::OnBreak { since, worked_seconds: 5400 } if since == break_start
        ));

        workday::engine::end_workday(&mut conn, now()).unwrap();
        assert_eq!(
            snapshot(&conn, now()).unwrap().workday,
            WorkdayStatus::Ended { worked_seconds: 5400 }
        );
    }

    #[test]
    fn action_ids_round_trip() {
        let actions = [
            TrayAction::StartTimer(42),
            TrayAction::StopTimer,
            TrayAction::StartWorkday,
            TrayAction::EndWorkday,
            TrayAction::StartBreak,
            TrayAction::EndBreak,
            TrayAction::Sync,
            TrayAction::OpenWindow,
            TrayAction::Quit,
        ];
        for action in actions {
            assert_eq!(TrayAction::parse(&action.id()), Some(action));
        }
        assert_eq!(TrayAction::parse("start:abc"), None);
        assert_eq!(TrayAction::parse("bogus"), None);
    }

    #[test]
    fn labels() {
        assert_eq!(format_hm(0), "0:00");
        assert_eq!(format_hm(3 * 3600 + 5 * 60 + 59), "3:05");
        assert_eq!(escape("R&D"), "R&&D");

        let long = TaskRef {
            id: 1,
            key: "PROJ-1".into(),
            summary: "x".repeat(60),
        };
        let label = task_label(&long);
        assert!(label.ends_with('…'));
        assert_eq!(label.chars().count(), "PROJ-1  ".len() + SUMMARY_MAX_CHARS);

        let model = TrayModel {
            running: Some(RunningTimer {
                task: TaskRef {
                    id: 1,
                    key: "PROJ-1".into(),
                    summary: "Fix login".into(),
                },
                started_at: now() - chrono::Duration::minutes(83),
            }),
            ..TrayModel::default()
        };
        assert_eq!(timer_label(&model, now()), "● PROJ-1 · 1:23 — Fix login");
        assert_eq!(timer_label(&TrayModel::default(), now()), "No timer running");

        assert_eq!(sync_label(3, false), "Sync to Jira (3 pending)");
        assert_eq!(sync_label(0, false), "Sync to Jira (all synced)");
        assert_eq!(sync_label(3, true), "Syncing…");
    }
}
