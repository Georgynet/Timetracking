# 0037: macOS menu bar extras — title, state icons, left click, close-to-tray

**Status:** Accepted — 2026-10-26

## Context

ADR-0036 gave the tray a status + quick-actions menu on both platforms. On macOS the
menu bar can do more, and that's where the app is primarily used: an always-visible
timer, an icon that reflects state, and the usual menu bar utility behavior of living on
after its window is closed. On Linux these are unreliable — AppIndicator has no click
events, label support varies by desktop environment, and the tray itself may be
missing (ADR-0010) — and can't be verified, so Linux keeps the plain menu.

## Decision

macOS only:

- **Title next to the icon** (`tray::menu_bar_title`): `PROJ-123 1:23` while a timer
  runs, `Break 0:12` on a break without a timer, nothing while idle. Updated on every
  rebuild and by the 30-second ticker.
- **State icons**: three template images — `tray-idle` (stopwatch hand), `tray-running`
  (filled sector; the former `tray-template` from ADR-0035, renamed) and `tray-break`
  (pause bars) — sources in `src-tauri/icons/source/`, rendered at 36×36 as described in
  ADR-0035. A running timer wins over a break (`IconState::of`), since a timer left
  running during a break is what the user should notice. Swapped on menu rebuilds;
  `set_icon_as_template(true)` is re-applied after each `set_icon`, which resets it.
- **Left click opens the window**, right click shows the menu
  (`show_menu_on_left_click(false)` + `on_tray_icon_event`).
- **Closing the window hides it** instead of quitting, when the tray is available
  (`on_window_event` in `lib.rs`); "Quit" in the tray or app menu (⌘Q) exits. Clicking
  the Dock icon with no visible window (`RunEvent::Reopen`) shows it again.

Linux keeps ADR-0036's menu, the color app icon, and quit-on-close.

## Consequences

- The running timer is visible at all times on macOS without opening anything.
- Closing the window no longer stops the app on macOS, so the tray (and a running
  timer's live title) keeps working; quitting is an explicit action.
- Two platform behaviors to keep in mind when touching window or tray code; the
  `cfg!(target_os = "macos")` checks are confined to `tray.rs` and the close handler.

## Alternatives considered

- **Same extras on Linux** — rejected: unverifiable there and partly unsupported
  (clicks), and hiding the window on close with an unreliable tray could leave the user
  with no way back to it.
- **Seconds in the title** — rejected: a per-second tick for a menu bar label is
  distracting and needs a much more frequent timer; minutes match the menu's status.
- **Menu on left click (macOS default)** — rejected: the menu remains one right click
  away, and a left click opening the window is the quickest path to the full UI.
