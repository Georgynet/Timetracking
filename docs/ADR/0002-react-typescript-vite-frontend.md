# 0002: React + TypeScript + Vite for the frontend

**Status:** Accepted — 2026-08-10

## Context

The spec leaves the frontend framework as "team's choice" within Tauri's Rust +
web-frontend architecture. The realistic options for a Tauri v2 app are React, Svelte,
or Vue, all offered as official `create-tauri-app` templates. This is a small,
single-developer, single-window CRUD-plus-timer app — the choice matters mainly for
day-to-day ergonomics and the amount of example code/tooling available, not for
scaling concerns.

## Decision

**React + TypeScript**, scaffolded via `create-tauri-app`'s official template, bundled
with **Vite**. State is a single `zustand` store (`src/state/store.ts`) rather than
React Context or Redux — the app's shared state (settings, task lists, active timer,
unsynced count) is small and flat enough that a heavier state library isn't justified.
No data-fetching library (e.g. `@tanstack/react-query`) is used either — mutations are
followed by an explicit refetch of the relevant store slice, which is simple enough at
this scale.

## Consequences

- Largest ecosystem and most Tauri-specific example code of the three options, which
  matters for a project one person will maintain and periodically pick back up.
- `Task` and `TimeEntry` DTOs are defined once in Rust
  (`#[serde(rename_all = "camelCase")]`) and mirrored by hand in
  `src/api/types.ts` — there is no shared-type-generation step, so the two must be
  kept in sync manually when a field changes.

## Alternatives considered

- **Svelte** — lighter weight and less boilerplate, but a smaller ecosystem and fewer
  Tauri-specific examples; rejected in favor of the more common pairing.
- **Vue** — solid middle ground, but less common in the Tauri community than React;
  no specific advantage for this app's needs.
