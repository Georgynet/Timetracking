import { confirm } from "@tauri-apps/plugin-dialog";
import { FormEvent, useState } from "react";
import { clearJiraSettings } from "../api/commands";
import type { Preferences, TicketOrder } from "../api/types";

interface SettingsModalProps {
  preferences: Preferences;
  /** Shown beside Log out, so it's clear which account is about to be disconnected. */
  jiraEmail: string | null;
  onClose: () => void;
  onSave: (next: Preferences) => Promise<void>;
  /** Called once the token is cleared, to send the app back to Setup. */
  onLoggedOut: () => void;
}

/**
 * App preferences — panel heights, the sprint-filter default and picker ordering so
 * far. The shape is built to grow, since the backing store is a key/value table
 * rather than columns (see ADR-0026).
 */
export function SettingsModal({
  preferences,
  jiraEmail,
  onClose,
  onSave,
  onLoggedOut,
}: SettingsModalProps) {
  const [myTasksRows, setMyTasksRows] = useState(preferences.myTasksRows);
  const [favoritesRows, setFavoritesRows] = useState(preferences.favoritesRows);
  const [currentSprintDefault, setCurrentSprintDefault] = useState(preferences.currentSprintDefault);
  const [ticketOrder, setTicketOrder] = useState<TicketOrder>(preferences.ticketOrder);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [loggingOut, setLoggingOut] = useState(false);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setSaving(true);
    setError(null);
    try {
      await onSave({ myTasksRows, favoritesRows, currentSprintDefault, ticketOrder });
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
      "Log out of Jira? Your tracked time stays on this machine — you'll need to enter your API token again to sync.",
      { title: "Log out", kind: "warning" },
    );
    if (!confirmed) return;
    setLoggingOut(true);
    setError(null);
    try {
      await clearJiraSettings();
      onLoggedOut();
    } catch (err) {
      setError(err as string);
      setLoggingOut(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
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
        <h3 className="settings-group">Jira connection</h3>
        <div className="settings-account">
          <span className="settings-account-email">{jiraEmail ?? "Not connected"}</span>
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
          <button type="button" className="link-button" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" disabled={saving}>
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </form>
    </div>
  );
}
