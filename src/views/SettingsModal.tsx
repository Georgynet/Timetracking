import { confirm } from "@tauri-apps/plugin-dialog";
import { FormEvent, useState } from "react";
import { clearJiraSettings } from "../api/commands";
import type { Preferences, ThemePreference, TicketOrder } from "../api/types";

interface SettingsModalProps {
  preferences: Preferences;
  /** Warns before logging out, since Setup has no timer UI to stop it from. */
  timerRunning: boolean;
  /** Shown beside Log out, so it's clear which account is about to be disconnected. */
  jiraEmail: string | null;
  onClose: () => void;
  onSave: (next: Preferences) => Promise<void>;
  /** Called once the token is cleared, to send the app back to Setup. */
  onLoggedOut: () => void | Promise<void>;
}

/**
 * App preferences — panel heights, the sprint default, picker ordering and theme so
 * far. The shape is built to grow, since the backing store is a key/value table
 * rather than columns (see ADR-0026).
 */
export function SettingsModal({
  preferences,
  timerRunning,
  jiraEmail,
  onClose,
  onSave,
  onLoggedOut,
}: SettingsModalProps) {
  const [myTasksRows, setMyTasksRows] = useState(preferences.myTasksRows);
  const [favoritesRows, setFavoritesRows] = useState(preferences.favoritesRows);
  const [currentSprintDefault, setCurrentSprintDefault] = useState(preferences.currentSprintDefault);
  const [ticketOrder, setTicketOrder] = useState<TicketOrder>(preferences.ticketOrder);
  const [theme, setTheme] = useState<ThemePreference>(preferences.theme);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [loggingOut, setLoggingOut] = useState(false);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setSaving(true);
    setError(null);
    try {
      await onSave({ myTasksRows, favoritesRows, currentSprintDefault, ticketOrder, theme });
      onClose();
    } catch (err) {
      setError(err as string);
    } finally {
      setSaving(false);
    }
  }

  /** Deliberately immediate rather than staged behind Save: this isn't a preference, and
   *  a logout that only took effect on Save would be easy to leave half-done. */
  async function handleLogout() {
    const confirmed = await confirm(
      timerRunning
        ? "A timer is still running. Logging out returns to setup, where there is no way to stop it — it will keep counting until you log back in. Log out anyway?"
        : "Log out of Jira? Your tracked time stays on this machine — you'll need to enter your API token again to sync.",
      { title: "Log out", kind: "warning" },
    );
    if (!confirmed) return;
    setLoggingOut(true);
    setError(null);
    try {
      await clearJiraSettings();
      // Awaited: it re-reads settings, and a failure there would otherwise be an
      // unhandled rejection that leaves this button stuck on "Logging out…".
      await onLoggedOut();
    } catch (err) {
      setError(err as string);
      setLoggingOut(false);
    }
  }

  // Nothing may dismiss the dialog mid-logout: unmounting it would strand the request
  // with no place left to report a failure, and a failed logout is not a no-op.
  const busy = saving || loggingOut;

  return (
    <div className="modal-backdrop" onClick={busy ? undefined : onClose}>
      <form className="modal" onClick={(e) => e.stopPropagation()} onSubmit={handleSubmit}>
        <h2>Settings</h2>
        <h3 className="settings-group">Task panels</h3>
        <label>
          My Tasks rows
          <input
            type="number"
            min={1}
            max={25}
            value={myTasksRows}
            onChange={(e) => setMyTasksRows(Number(e.target.value))}
            required
          />
        </label>
        <label>
          Favorites rows
          <input
            type="number"
            min={1}
            max={25}
            value={favoritesRows}
            onChange={(e) => setFavoritesRows(Number(e.target.value))}
            required
          />
        </label>
        <p className="field-hint">
          How many entries each panel shows before it scrolls. Nothing is hidden — a
          taller panel just pushes the timer and History further down the page.
        </p>
        <label className="settings-check">
          <input
            type="checkbox"
            checked={currentSprintDefault}
            onChange={(e) => setCurrentSprintDefault(e.target.checked)}
          />
          Start with "Current sprint" ticked
        </label>
        <h3 className="settings-group">Ticket picker</h3>
        <label>
          Order
          <select value={ticketOrder} onChange={(e) => setTicketOrder(e.target.value as TicketOrder)}>
            <option value="recent">Recently tracked first</option>
            <option value="key">Ticket key (A–Z)</option>
          </select>
        </label>
        <p className="field-hint">
          Applies to the pickers in the timer and the entry dialogs. Tickets you have
          never tracked come last, in key order.
        </p>
        <h3 className="settings-group">Appearance</h3>
        <label>
          Theme
          <select value={theme} onChange={(e) => setTheme(e.target.value as ThemePreference)}>
            <option value="system">Follow system</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </label>
        <p className="field-hint">
          Following the system switches with macOS, including its automatic day/night
          schedule.
        </p>
        <h3 className="settings-group">Jira connection</h3>
        <div className="settings-account">
          <span className="settings-account-email" title={jiraEmail ?? undefined}>
            {jiraEmail ?? "Not connected"}
          </span>
          {/* `type="button"`: inside a form, the default would submit it. */}
          <button type="button" className="link-button danger" onClick={handleLogout} disabled={loggingOut}>
            {loggingOut ? "Logging out…" : "Log out"}
          </button>
        </div>
        <p className="field-hint">
          Logging out clears the API token from the keychain and returns to setup. Time
          entries and favorites are kept — they live in the local database, not in Jira.
        </p>
        {error && <p className="error">{error}</p>}
        <div className="modal-actions">
          <button type="button" className="link-button" onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button type="submit" disabled={busy}>
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </form>
    </div>
  );
}
