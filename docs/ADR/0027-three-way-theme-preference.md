# 0027: A three-way theme preference, resolved in JS to a `data-theme` attribute

**Status:** Accepted — 2026-08-22

## Context

The app was light-only in practice. A partial `@media (prefers-color-scheme: dark)`
block existed — it redefined the background, panel, border, muted and chart colours —
but it never redefined `--accent`, `--danger`, `--pending` or `--synced`, and nothing
declared `color-scheme`. The result on a dark system was a dark page with light native
controls (every text input, select, checkbox, radio, date/time picker and scrollbar)
and accent-blue text at 3.1:1 against the dark panel, well under the 4.5:1 needed to
read comfortably.

Following the system is also not enough on its own: someone may want this app dark on
a light desktop, and on Linux the OS scheme is known not to reach the webview
reliably, which would leave those users with no way to get dark at all.

## Decision

**The preference is three-way — Follow system / Light / Dark — but the DOM only ever
carries the resolved scheme.** `data-theme="light" | "dark"` on the root element, never
`"system"`. `src/theme.ts` resolves `"system"` via `matchMedia` and keeps a `change`
listener attached while that setting is active, so flipping macOS — including its
automatic day/night schedule — repaints the app live.

Resolving in JS rather than in CSS is what keeps the dark palette written **once**.
The alternatives fail on exactly that: a media query plus an attribute selector
(`@media … { :root:not([data-theme="light"]) }` alongside `:root[data-theme="dark"]`)
cannot share a declaration block, so the ~20 values appear twice and drift; and
`light-dark()`, which would express it natively, is unsupported below WebKit 17.5
(macOS < 14.5, WebKitGTK < 2.46), where it does not degrade but invalidates every
colour it touches.

Supporting decisions:

- **One `MediaQueryList` for the module's lifetime.** `window.matchMedia()` returns a
  *new* object on every call, and a listener can only be removed through the object it
  was added to — so a per-call `matchMedia()` would leak a listener on every apply, and
  those leaked listeners would keep rewriting `data-theme` from the OS scheme after the
  user explicitly chose Light or Dark.
- `color-scheme` is set from the same attribute (`light` on `:root`, `dark` in the dark
  block), which is what makes native controls and scrollbars render darkly, plus
  `accent-color: var(--accent)` so checkboxes and radios match the app rather than the
  OS default blue.
- A new `--accent-contrast` token for text drawn *on* an accent fill. No single blue
  works both as text on a dark background and as a fill under white text: dark mode
  lightens `--accent` to `#7aa2ec` (6.0:1 as text) and flips the text on it to
  `#1a1a1a` (6.8:1). `--danger` (`#ea766a`), `--pending` (`#d9a521`) and `--synced`
  (`#66bb6a`) are likewise lightened, each checked against the surface it actually sits
  on — including the page background, where setup, header-sync and statistics errors
  render, and the badge backgrounds, where the old values ran as low as 2.4:1.
  `--chart-gridline` becomes `#3f3f3c`: it must be *lighter* than the panel it is drawn
  on, the inverse of the light theme's rule. `--chart-axis-label` is lifted from the
  light theme's `#898781` to `#9b9992`, which takes an 11px label from a marginal
  4.3:1 to 5.4:1 — small text is where a borderline value actually hurts.
- **The theme is applied before the first paint, from `index.html`.** An inline style
  and script there read the same `localStorage` key `theme.ts` writes on every apply.
  The real value lives in the `preferences` table (ADR-0025) but arrives
  asynchronously, and the webview paints its default white before any of the app's CSS
  or JS exists — so without this an explicitly-dark user gets a white frame at every
  launch. The duplication of a few values between `index.html`, `theme.ts` and
  `App.css` is deliberate: nothing imported can run that early.
- `getCurrentWindow().setTheme()` keeps the native title bar in step, which needs
  `core:window:allow-set-theme` in the window capability — it is not part of
  `core:default`, and without it every call is denied by the ACL. The promise carries
  its own `.catch`, because a synchronous `try`/`catch` covers only the
  `getCurrentWindow()` throw in a plain browser, not an async IPC rejection.
- `loadPreferences` moved from `MainView` to `App`, so the theme also applies on the
  setup screen — the first thing a new user sees.

## Consequences

- The stylesheet no longer reacts to the OS on its own: with JS disabled or failing
  before `initTheme`, the app renders light regardless of system setting. Acceptable in
  a desktop app whose entire UI is React — nothing renders at all in that case.
- On Linux, "Follow system" is expected to resolve to light even on a dark desktop
  (Tauri/wry do not propagate the scheme to WebKitGTK). The explicit Light/Dark choice
  is unaffected and is precisely the escape hatch a CSS-only design would not have had.
- Every colour decision now lives in two blocks at the top and bottom of `App.css`.
  The audit behind this ADR found only nine literals outside them (accent-fill text,
  the three badge backgrounds, the modal backdrop); the accent-fill ones became the new
  token, and the rest got dark overrides. New CSS must keep using tokens or dark mode
  silently regresses — there is no test that would catch it.
- Two surfaces needed dark-only treatment beyond tokens: the modal backdrop deepens to
  60% (40% black over a dark page barely separates it) and the modal gains a border,
  since the shadowless panel would otherwise blend into the scrim.

## Alternatives considered

- **System-following only, no override** — one less setting, but it cannot serve
  someone who wants this app dark on a light desktop, and it strands Linux users
  entirely; the cost of the override is a `<select>` and one key in a table that
  already exists.
- **A `theme` class on `<body>`** — equivalent in effect; the attribute is
  self-describing in the inspector and reads naturally in selectors
  (`[data-theme="dark"] .badge-synced`).
- **A semantic token layer** (`--text`, `--bg`, `--hover-bg`) — a cleaner vocabulary,
  but it would touch nearly every rule in the file for no additional correctness, since
  the existing tokens already covered all but nine literals.
