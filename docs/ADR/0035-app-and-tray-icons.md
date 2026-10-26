# 0035: Own app icon, and a monochrome template icon for the macOS tray

**Status:** Accepted — 2026-10-26

## Context

`src-tauri/icons/` still held the stock Tauri logo from project scaffolding, so the Dock,
the app bundle and the system tray all showed an icon unrelated to the app. The tray
reused `default_window_icon()`, i.e. that same full-color image. On macOS, menu bar items
are expected to be monochrome "template" images that the OS recolors for light and dark
menu bars; a color icon there looks foreign.

## Decision

- Keep the icon sources as SVG in `src-tauri/icons/source/`: `app-icon.svg` (a stopwatch
  on a blue rounded square, blue matching the UI accent) and `tray-template.svg` (the same
  stopwatch as a black silhouette on transparency).
- Generate the app icon set from `app-icon.svg` with `npx tauri icon` (run from
  `src-tauri/`; delete the `android/`, `ios/` and `64x64.png` outputs it also creates,
  since this is a desktop-only app).
- Render `tray-template.svg` to `icons/tray-template.png` (36×36, i.e. the 18pt menu bar
  height at 2×) with `npx tauri icon icons/source/tray-template.svg -o <dir> --png 36`,
  and embed it at compile time via `tauri::include_image!`.
- `lib.rs::tray_icon` picks per platform: on macOS the template PNG with
  `icon_as_template(true)`; elsewhere the regular app icon, because Linux panels have no
  template convention and a black glyph would vanish on a dark panel.

## Consequences

- The app is recognizable in the Dock, the bundle and the tray, and the macOS tray icon
  follows the system's light/dark menu bar automatically.
- Changing the icon means editing the SVG and re-running the two commands above; the
  generated PNG/ICNS/ICO files are build artifacts checked in for convenience.
- Prepares for state-dependent tray icons (idle / running / break) on macOS, which can be
  further template SVGs in the same folder.

## Alternatives considered

- **Same color icon in the tray on all platforms** — rejected: looks out of place in the
  macOS menu bar and ignores dark mode.
- **Template icon on Linux too** — rejected: no OS recoloring there, so it would be
  invisible on dark panels.
