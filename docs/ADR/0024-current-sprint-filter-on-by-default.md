# 0024: The "current sprint" filter is on by default, and My Tasks shows its count

**Status:** Accepted — 2026-08-21 — refines [0019](0019-current-sprint-filter-via-dual-jql.md)

## Context

ADR-0019 added the "Current sprint" toggle to My Tasks, defaulting to off so the panel
kept showing every assigned ticket until the user opted in. In practice the sprint is
what's being worked on essentially every time a timer is started, and the full
assigned list is long enough that the sprint's tickets are buried in it — the toggle
was being switched on at the start of every session.

The panel also gave no sense of how much it was listing, so the effect of the toggle
was only visible by scrolling.

## Decision

- The toggle starts **on**. It is still a plain component-local toggle, not a
  persisted setting: turning it off is a per-session look at the wider list, and it
  returns to the sprint view next launch.
- The heading carries the count of what is actually listed — `My Tasks (16)` — so it
  follows the filter rather than reporting the unfiltered total. Ticking the toggle
  changes the number along with the list, which is what makes the filter's effect
  legible.

## Consequences

- ADR-0019 notes that the sprint query is **best-effort**: it is a second JQL call, and
  when it fails (or the board simply has no active sprint) no task carries
  `is_in_current_sprint`. With the filter now on by default, that case shows an empty
  panel on launch rather than the full list it used to. The empty state names the way
  out — "untick the filter to see everything assigned to you" — which is the mitigation
  chosen over silently falling back to the unfiltered list, since a silent fallback
  would make a failing sprint query indistinguishable from a sprint with no tickets.
- The count is of the *filtered* list by design. `My Tasks (16)` with the toggle on and
  `My Tasks (41)` with it off is the intended behaviour, not a bug: both are counts of
  what is on screen.

## Alternatives considered

- **Persist the toggle in settings** — remembers a deliberate choice, but it needs a
  settings row, a command and a load path for a preference the user is unlikely to set
  more than once; the default now matches what that preference would almost always be.
- **Fall back to the unfiltered list when nothing is in the sprint** — hides a failed
  sprint query behind what looks like normal behaviour, and the panel would silently
  change meaning between launches.
- **Show both counts (`16 / 41`)** — more information than the heading needs, and the
  second number is one click away.
