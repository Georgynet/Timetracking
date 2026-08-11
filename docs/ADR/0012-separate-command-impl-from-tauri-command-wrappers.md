# 0012: Separate pure command-impl functions from `#[tauri::command]` wrappers for testability

**Status:** Accepted — 2026-08-10

## Context

`#[tauri::command]` functions take `tauri::State<'_, T>` as their state parameter.
Constructing a real `State<T>` in a unit test requires spinning up a full mock Tauri
`App` via `tauri::test::mock_builder()`/`mock_context()`, which in turn requires
enabling Tauri's `test` crate feature — extra build-time dependency surface and
scaffolding whose only purpose would be satisfying a type signature, not exercising
any Tauri-specific behavior.

## Decision

Command modules that have meaningful logic worth unit-testing (`commands::entries`,
notably — it contains the trickiest business logic in the app: bounds resolution for
manual entries, the start-time-shift-preserves-duration rule, and the delete-eligibility
guard from ADR-0009) split each command into:

- a plain function taking `&AppState` directly (e.g. `create_manual_entry_impl`,
  `update_time_entry_impl`, `delete_draft_entry_impl`), containing all the actual
  logic, and
- a thin `#[tauri::command]`-annotated wrapper that unwraps `State<'_, AppState>` into
  a `&AppState` and delegates to the `_impl` function.

Tests construct an `AppState` directly (`AppState::new(open_in_memory().unwrap())`,
no Tauri runtime involved at all) and call the `_impl` functions. Modules whose logic
is thin enough to already be covered by the underlying repo/engine/service layer's own
tests (`commands::setup`, `commands::tasks`, `commands::timer`, `commands::sync`) skip
this split — the command function itself just wires `State` to already-tested
lower-level calls, so there is nothing additional worth testing at the command layer
there.

## Consequences

- `tauri`'s `test` feature is never enabled, and no test in the project depends on
  Tauri's mock-app machinery.
- The pattern is not applied uniformly to every command module — only where the
  command layer itself contains non-trivial logic. A future command that grows real
  logic of its own should get the same `_impl` split rather than being tested only
  indirectly.
- Adds one extra layer of indirection (a thin wrapper) in the modules where it's
  used, in exchange for tests that run in milliseconds with no mock-runtime setup.

## Alternatives considered

- **Enable `tauri`'s `test` feature and use `tauri::test::mock_builder()` to construct
  a real `State<AppState>` in tests** — tried initially, then rejected: pulls in a
  full mock `App`/webview construction path for tests that only need `&AppState`, and
  adds a permanent feature-flag dependency for no behavioral coverage Tauri itself is
  responsible for.
