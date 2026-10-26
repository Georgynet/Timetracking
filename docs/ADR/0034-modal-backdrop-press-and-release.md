# 0034: Close a modal on backdrop click only when press and release both land on it

**Status:** Accepted — 2026-10-26

## Context

Every modal (`ManualEntryForm`, `EditWorkdayForm`, `EditBreakForm`, `SettingsModal`,
`SyncReportModal`) closed via `onClick={onClose}` on `.modal-backdrop`, with the inner
`.modal` calling `stopPropagation()`. A `click` event fires on the nearest common
ancestor of the `mousedown` and `mouseup` targets, so a drag that starts inside the
modal — typically selecting the old value in a duration/time input — and is released
over the backdrop dispatched `click` on the backdrop and closed the modal, discarding
whatever had been typed. This happened often enough in the manual entry form to be a
real usability problem.

## Decision

Add a shared hook, `useBackdropClose(onClose)` in `src/lib/modal.ts`, that returns
`onMouseDown`/`onClick` props for the backdrop. `onMouseDown` records whether the press
landed on the backdrop itself (`e.target === e.currentTarget`); `onClick` closes only if
that press flag is set *and* the click target is the backdrop itself. All five modals
spread these props onto their backdrop; the inner `stopPropagation()` calls are no
longer needed (the target check already ignores clicks inside the modal) and were
removed. Passing `undefined` keeps the backdrop inert, which `SettingsModal` uses while
a save/logout is in flight.

## Consequences

- Dragging from inside a modal to outside it no longer closes it; a deliberate
  click on the backdrop still does.
- The rule lives in one place, so a future modal gets the right behavior by using the
  hook rather than re-deriving the `onClick` pattern.

## Alternatives considered

- **Never close on backdrop click** (Cancel / explicit close only) — rejected: closing a
  modal by clicking outside is expected behavior and harmless for the read-only
  `SyncReportModal`; the problem was only the drag case.
- **Listen to `mousedown` on the backdrop instead of `click`** — rejected: it would
  close on press, before the user can drag away to cancel, which is the inverse of how
  native click handling works.
