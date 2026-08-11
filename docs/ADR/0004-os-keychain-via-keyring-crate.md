# 0004: Store the Jira API token in the OS keychain via the `keyring` crate

**Status:** Accepted — 2026-08-10

## Context

The spec requires the Jira API token to live in OS-protected secret storage (macOS
Keychain / Linux Secret Service), never in the plaintext SQLite database. Rust has one
maintained cross-platform crate for this: `keyring`.

## Decision

Use the **`keyring`** crate (v4, its `v1`-compatible `Entry` API) with the
`apple-native-keyring-store` feature explicitly enabled for macOS — `keyring` v4's
default features cover Windows and Linux (via `zbus-secret-service-keyring-store`) but
*not* Apple's native backend, so it must be turned on explicitly for this app's
primary platform. `SERVICE_NAME` in `secrets/keyring_store.rs` matches
`tauri.conf.json`'s `identifier` (`com.georg.timetracing`) to namespace the entry to
this app.

The `settings` SQLite table holds only `jira_base_url`/`jira_email` (needed at startup
to decide Setup-vs-Main view before touching the keychain at all); the token itself
never has a column and is never constructed into any struct that could plausibly be
logged (see ADR-0011). `save_jira_settings` writes the keychain entry only after a
real `GET /myself` call has validated the credentials, and rolls back the settings-row
write if the keychain write subsequently fails, so the app never ends up with a
settings row and no token or vice versa.

## Consequences

- On Linux, this depends on a running Secret Service provider (gnome-keyring,
  KWallet). A minimal/headless Linux setup without one will fail here **by design** —
  the spec mandates OS secret storage, so there is no silent plaintext fallback. The
  error message in `SecretError::Backend` calls this out specifically rather than
  giving a generic failure.
- Verified on macOS with a real (non-mocked) Keychain round-trip test
  (`secrets::keyring_store::tests::real_keychain_round_trip`, `#[ignore]`d by default
  since it touches real OS state — run explicitly with `cargo test -- --ignored`).
  The Linux Secret Service path is unverified (no Linux machine available this
  session) — see ADR-0005 for the same caveat applied to Jira itself.

## Alternatives considered

- **Tauri's `stronghold` plugin** (an encrypted vault file, not OS-native) — would
  sidestep Linux Secret Service availability, but the spec explicitly asks for OS
  Keychain/Secret Service, not an app-managed vault; rejected as not matching the
  stated requirement.
