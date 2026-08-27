# 0028: Disconnecting from Jira lives in Settings and is called "Log out"

**Status:** Accepted — 2026-08-26

## Context

Disconnecting from Jira sat in the header as `Reconfigure`, a `link-button` beside
`Settings` and the primary `Sync`. ADR-0026 recorded that placement deliberately — the
two link-buttons "read as a pair beside the primary Sync button" — but the pairing was
the only thing they had in common. `Settings` opens a dialog you can back out of;
`Reconfigure` clears a credential. One is used constantly, the other perhaps twice in
the app's life, and they sat a few pixels apart.

The name was wrong too: it described the dialog you land in rather than what the action
does. Raised in review on PR #23.

## Decision

The action moves into the Settings modal, under a `Jira connection` heading beside the
address it would disconnect, and is called `Log out`.

- **It stays immediate, not staged behind `Save`.** The modal is otherwise a form: you
  change preferences and they apply on submit. A logout that only took effect on Save
  would be a trap — dismissing the dialog afterwards would leave it ambiguous whether you
  had logged out. It is therefore `type="button"` (the default inside a `<form>` would
  submit it) and runs on click, behind a native `confirm`.
- **The confirm says what survives.** The token goes; entries, favorites and workdays
  stay, because they live in the local database and never depended on the connection.
- **It warns when a timer is running.** Setup has no timer UI, so logging out mid-timer
  leaves an entry accruing with no way to stop it until you log back in. The dialog says
  so rather than silently stopping the timer, which would be a different decision about
  who owns the timer's lifecycle (ADR-0006).
- **Nothing can dismiss the dialog while the logout is in flight** — not Cancel, not the
  backdrop. Unmounting it would strand the request with nowhere to report a failure, and
  a failed logout is not a no-op (see below).
- **`clear_jira_settings` deletes the token before the settings row.** The old order
  cleared settings first, so a keychain failure — a locked keychain, or no Secret Service
  provider on Linux — left the credential behind with nothing pointing at it: the session
  kept working, and the next launch found no settings and dropped to Setup with the token
  stranded. Token-first means every partial failure leaves the previous state intact,
  matching the rollback `save_jira_settings` already does.

## Consequences

- The header loses a button, which is the point: what remains is the view tabs, `Sync`
  and `Settings`.
- ADR-0026's note about the two link-buttons reading as a pair is now historical. That
  ADR is not edited — its actual decision (preferences in a key/value table, behind a
  modal) stands unchanged; only the placement aside is superseded, by this record.
- The Settings modal now holds one action that is not a preference. That is a widening of
  its remit, accepted because "the Jira connection" is what a user looks in Settings for,
  and the alternative — a third home for a single button — is worse.
- Logging out does not cancel an in-flight sync: `sync_all` clones the client up front,
  so a batch already running finishes and posts its worklogs after the app has returned
  to Setup. Local state stays consistent, but the credential is still in use for a few
  seconds after "Log out" succeeds. Left alone deliberately; cancelling mid-batch would
  need the per-record isolation of ADR-0008 rethought.

## Alternatives considered

- **Leave it in the header, rename only.** Answers half the objection. The real problem
  is a destructive action sharing a corner with the button pressed all day.
- **Stage it behind `Save` like every other control in the dialog.** Consistent with the
  form, but it makes a credential-clearing action reversible-looking and defers it behind
  a second click that the user may never make.
- **Stop the running timer as part of logging out.** Tidier than a warning, and probably
  right eventually — but it takes the timer's lifecycle out of `timer::engine`'s hands
  (ADR-0006), and quietly ending someone's tracked work as a side effect of a different
  action needs its own decision.
- **A separate "Account" screen.** More room to grow, but a whole view for one button and
  one address.
