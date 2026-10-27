# 0036: The tray menu shows status and offers quick actions

**Status:** Accepted — 2026-10-26

## Context

The tray icon (ADR-0010, ADR-0035) only offered "Quit", so it carried no meaning. The
most frequent interactions with the app — checking what's running, switching tickets,
stepping away for a break — needed the main window. The spec asks for the active timer
to be visible "in the menu bar/tray, if technically feasible".

## Decision

`src-tauri/src/tray.rs` owns the tray. Its menu, identical on macOS and Linux:

- Disabled status lines: running timer (`● PROJ-123 · 1:23 — summary` / "No timer
  running") and workday state (working/on break since, worked today).
- "Stop timer", or "Resume <last tracked ticket>" while idle.
- "Start timer" / "Switch to" submenu: favorites, most recently tracked first, then up
  to five recently tracked non-favorites; the running ticket is left out.
- Workday actions valid for the current state (start workday; start/end break; end
  workday).
- "Sync to Jira (N pending)", run in the background, its report emitted to the
  frontend, which shows it in the usual sync report modal.
- "Open Time Tracker", "Quit".

Mechanics:

- A pure `snapshot(conn, now) -> TrayModel` reads everything the menu shows; label
  formatting and menu-id parsing (`TrayAction`) are pure too, and unit tested.
- The menu is **rebuilt** (`tray::refresh`) after tray actions and after the UI commands
  that change its structure: timer start/stop, workday start/end, break start/end,
  favorite add/remove. Between rebuilds a 30-second ticker only **updates texts**
  (`tray::update_texts`) of the status and sync items, so elapsed times stay current
  without swapping the menu out while it's open. Changes that only affect text (e.g.
  editing an entry changes the unsynced count) are picked up by the ticker.
- Tray actions call the same engines as the commands (`timer::engine`,
  `workday::engine`, `commands::sync::sync_all_impl`), so all rules (single running
  timer, break/workday checks) apply unchanged, then emit `tray-state-changed`; the
  frontend (`src/api/events.ts`, `MainView`) refetches state as after its own actions.
- Literal `&` in labels is doubled, since menu labels treat it as a mnemonic marker.

macOS-only extras (title text next to the icon, state icons, click behavior,
notifications) are deliberately left for separate decisions.

## Consequences

- The common actions no longer need the window; the UI stays consistent with changes
  made from the tray.
- Commands that affect the menu's structure must call `tray::refresh` after releasing the
  DB lock; forgetting it leaves the menu stale until the next rebuild (texts still
  refresh via the ticker).
- Status texts can lag by up to 30 seconds.

## Alternatives considered

- **Rebuild the whole menu on every tick** — rejected: replacing a menu while it's open
  can close or glitch it, and nothing structural changes on a timer.
- **Let the frontend tell the backend to refresh the tray** — rejected: the tray must
  stay correct even when the window is closed or the frontend skips a call; the backend
  already knows when state changes.
- **Only favorites in the start submenu** — rejected: one-off tickets tracked recently
  are often what you return to; "My tasks" is left out to keep the menu short.
