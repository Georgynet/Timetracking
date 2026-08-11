# 0010: Build the system tray programmatically with a window-only fallback for Linux

**Status:** Accepted — 2026-08-10

## Context

The spec asks for active-timer status in the tray/menu bar "if technically feasible,"
and explicitly flags that Linux tray behavior differs across desktop environments —
asking for either a minimally-supported-DE list or a universal fallback UI (open
question #3). Stock GNOME, in particular, has no tray/StatusNotifierItem support at
all without an extension (e.g. AppIndicator); KDE generally does support it natively.

## Decision

The tray icon and menu are built **programmatically** in `lib.rs`'s `setup` hook
(`build_tray`), not declared statically in `tauri.conf.json`, specifically so its
result (`Ok`/`Err`) can be caught. `AppState.tray_available` records whether it
succeeded, exposed to the frontend via the `is_tray_available` command. The frontend
(`HeaderBar`) checks this on load and, when the tray is unavailable, shows an inline
"tray unavailable — status shown here" note — the timer's running/stopped state is
always visible in the main window regardless (`TimerWidget`), so the fallback is just
making that visibility explicit rather than implementing a second, separate status
surface.

No attempt is made to detect *which* desktop environment is running or to special-case
GNOME vs. KDE — the same try/catch-and-fall-back logic runs unconditionally on both
platforms, which is simpler than DE-sniffing and degrades correctly regardless of which
Linux tray backend Tauri's `tray-icon` crate ends up using underneath.

## Consequences

- The app never crashes or fails to start due to tray unavailability — worst case is
  the fallback note in the header bar.
- On stock GNOME without an AppIndicator-compatible extension installed, the
  window-only fallback is expected to be the **primary** experience, not a rare edge
  case — this is called out explicitly in `README.md`'s "Known gaps" section, since it
  could not be verified on an actual Ubuntu/GNOME machine this session (see ADR-0004
  and ADR-0005 for the same "verified on macOS only" caveat applied elsewhere).
- If a future need arises to guide Linux users toward installing a tray extension,
  the `is_tray_available` command is already the hook point for a more detailed
  in-app message — no backend change would be needed, only frontend copy.

## Alternatives considered

- **Declare the tray statically in `tauri.conf.json`** — the more common pattern for
  apps that don't need a fallback path, but it offers no way to detect and react to
  tray-creation failure; rejected specifically because Linux tray support is known to
  be inconsistent per the spec's own framing of the problem.
- **Detect the desktop environment and only attempt a tray on known-good ones (e.g.
  KDE)** — rejected as more complex than needed; attempt-and-catch achieves the same
  outcome without a DE allowlist to maintain.
