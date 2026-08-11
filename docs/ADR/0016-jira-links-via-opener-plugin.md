# 0016: Link tickets to Jira via the `opener` plugin, not plain anchor navigation

**Status:** Accepted — 2026-08-11

## Context

The user asked for ticket keys in My Tasks, Favorites, and History to link directly to
the corresponding Jira issue. The app is a Tauri desktop webview, not a browser tab: a
plain `<a href="https://..." target="_blank">` is not guaranteed to open the system's
default browser — depending on the platform's webview (WKWebView, WebView2,
WebKitGTK) and Tauri's navigation policy, it can instead try to navigate the app's own
window away from the UI, or open a bare second webview window with no browser chrome.
The project already depends on `@tauri-apps/plugin-opener` (in `package.json` and
enabled as `opener:default` in `src-tauri/capabilities/default.json`) for exactly this
purpose, but nothing in the codebase used it yet.

Building the issue URL itself needs the configured Jira base URL
(`settings.jiraBaseUrl`), which `MainView` already holds but had only ever passed to
children as part of the `settings` object as a whole, typed `string | null` per
`SettingsDto`. `App.tsx` never renders `MainView` unless `jiraBaseUrl` is set (see the
`isConfigured` gate), so the null case can't occur here in practice, but nothing
upstream of `MainView` encoded that for the type checker.

## Decision

- Added `jiraIssueUrl(baseUrl, key)` in `src/lib/jira.ts` — a single-purpose helper
  (`${baseUrl trimmed}/browse/${key}`) mirroring the same trailing-slash trimming the
  Rust `reqwest_client` already does for its own base-URL handling, so both sides build
  URLs the same way.
- `MyTasksPanel`, `FavoritesPanel`, and `HistoryList` each render the ticket key as an
  `<a>` with a real `href` (so copy-link/middle-click still work) but intercept
  `onClick` with `preventDefault()` and call `openUrl()` from `@tauri-apps/plugin-opener`
  explicitly, rather than relying on the webview's default anchor-navigation behavior.
- `MainView`'s existing `if (!settings) return null` guard was widened to
  `if (!settings || !settings.jiraBaseUrl) return null`, narrowing `jiraBaseUrl` to a
  plain `string` for everything below it. This restates an invariant that already held
  (via `App.tsx`'s `isConfigured` check) rather than introducing a new one, and lets
  `jiraBaseUrl` be passed down as a required prop instead of threading a `| null` type
  through three components that can't actually receive `null`.
- Only the tracked-ticket lists got links (My Tasks, Favorites, History rows).
  `FavoritesPanel`'s transient Jira search-results dropdown was left as plain text —
  it's a picker for *adding* a favorite, not a record of a tracked ticket, and wasn't
  named in the request.

## Consequences

- Ticket links open reliably in the user's actual default browser across platforms,
  consistent with how the rest of the app already treats "leave the app" actions.
- Any future ticket-key display gets the same two-line pattern (`href` for
  affordance/copy, `onClick` routed through `openUrl`) — there's now one place
  (`jiraIssueUrl`) that knows the URL shape, so a change to Jira's URL scheme (e.g. a
  different `/browse/` convention for Jira Server vs. Cloud) is a one-file change.
- `MainView` now returns `null` (renders nothing) for the theoretically-possible-but-
  never-actually-reached case of configured settings with an empty `jiraBaseUrl`,
  identically to how it already handles `settings` being entirely absent — no new
  behavior, just a wider guard on the same early return.

## Alternatives considered

- **Plain `<a href="..." target="_blank">` with no interception** — rejected: behavior
  across Tauri's supported webviews isn't reliable enough to trust for something as
  routine as "open a link," and the app already carries the `opener` plugin specifically
  to avoid this.
- **`window.open(url)`** — same underlying reliability problem as a bare anchor; still
  routes through the webview's own navigation/popup handling rather than the OS.
- **Pass `settings` (or `settings.jiraBaseUrl` typed as `string | null`) straight
  through and have each of the three components guard against `null` itself** —
  rejected as needless repetition: the invariant is already established once, in
  `MainView`, so re-checking it three times downstream would just be defensive code for
  a state that cannot occur there.
