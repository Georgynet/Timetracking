import { confirm } from "@tauri-apps/plugin-dialog";
import { FormEvent, useEffect, useState } from "react";
import {
  clearJiraSettings,
  getNotificationSettings,
  notificationsSupported,
  saveNotificationSettings,
} from "../api/commands";
import type { NotificationSettings, Preferences, ThemePreference, TicketOrder } from "../api/types";
import { formatDurationInput, parseDurationInput } from "../lib/format";
import { useBackdropClose } from "../lib/modal";

/** The reminders that take a threshold, each paired with its on/off flag. */
const REMINDERS = [
  { flag: "longTimer", minutes: "longTimerMinutes", label: "Timer running for" },
  { flag: "idleWorkday", minutes: "idleWorkdayMinutes", label: "Clocked in with no timer for" },
  { flag: "longBreak", minutes: "longBreakMinutes", label: "Break lasting" },
  { flag: "forgotEndWorkday", minutes: "endWorkdayMinutes", label: "Workday still running after working" },
] as const;

type ThresholdKey = (typeof REMINDERS)[number]["minutes"];
type ThresholdInputs = Record<ThresholdKey, string>;

function thresholdInputs(n: NotificationSettings): ThresholdInputs {
  return {
    longTimerMinutes: formatDurationInput(n.longTimerMinutes),
    idleWorkdayMinutes: formatDurationInput(n.idleWorkdayMinutes),
    longBreakMinutes: formatDurationInput(n.longBreakMinutes),
    endWorkdayMinutes: formatDurationInput(n.endWorkdayMinutes),
  };
}

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
  const [workdayEditingEnabled, setWorkdayEditingEnabled] = useState(preferences.workdayEditingEnabled);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [loggingOut, setLoggingOut] = useState(false);
  // `null` until loaded, and for good where notifications aren't supported (Linux) —
  // the section is hidden then rather than offering settings that do nothing.
  const [notifications, setNotifications] = useState<NotificationSettings | null>(null);
  const [thresholds, setThresholds] = useState<ThresholdInputs | null>(null);

  useEffect(() => {
    notificationsSupported()
      .then(async (supported) => {
        if (!supported) return;
        const loaded = await getNotificationSettings();
        setNotifications(loaded);
        setThresholds(thresholdInputs(loaded));
      })
      .catch((err) => setError(String(err)));
  }, []);

  /** Parses the threshold inputs back into minutes. A disabled reminder keeps its old
   *  value if its input doesn't parse, so a half-typed value can't block saving. */
  function resolveNotifications(): NotificationSettings | string {
    if (!notifications || !thresholds) return "Notification settings are not loaded.";
    const resolved = { ...notifications };
    for (const r of REMINDERS) {
      const parsed = parseDurationInput(thresholds[r.minutes]);
      if (parsed === null && notifications[r.flag]) {
        return `"${r.label}": enter a duration like 30m, 1h 30m or 2h`;
      }
      resolved[r.minutes] = parsed ?? notifications[r.minutes];
    }
    return resolved;
  }

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    const resolvedNotifications = notifications ? resolveNotifications() : null;
    if (typeof resolvedNotifications === "string") {
      setError(resolvedNotifications);
      return;
    }
    setSaving(true);
    setError(null);
    try {
      if (resolvedNotifications) await saveNotificationSettings(resolvedNotifications);
      await onSave({
        myTasksRows,
        favoritesRows,
        currentSprintDefault,
        ticketOrder,
        theme,
        workdayEditingEnabled,
      });
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

  const backdrop = useBackdropClose(busy ? undefined : onClose);

  return (
    <div className="modal-backdrop" {...backdrop}>
      <form className="modal" onSubmit={handleSubmit}>
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
        <h3 className="settings-group">Workday</h3>
        <label className="settings-check">
          <input
            type="checkbox"
            checked={workdayEditingEnabled}
            onChange={(e) => setWorkdayEditingEnabled(e.target.checked)}
          />
          Allow editing past workday sessions
        </label>
        <p className="field-hint">
          Adds Prev/Next day navigation and a sessions list to the workday widget, for
          fixing a forgotten clock-out from an earlier day. Off by default since it costs
          two rows of space in the widget and the need for it comes up rarely.
        </p>
        {notifications && thresholds && (
          <>
            <h3 className="settings-group">Notifications</h3>
            <label className="settings-check">
              <input
                type="checkbox"
                checked={notifications.syncResult}
                onChange={(e) => setNotifications({ ...notifications, syncResult: e.target.checked })}
              />
              Result of a sync started from the tray
            </label>
            <label className="settings-check">
              <input
                type="checkbox"
                checked={notifications.unsyncedOnEndWorkday}
                onChange={(e) =>
                  setNotifications({ ...notifications, unsyncedOnEndWorkday: e.target.checked })
                }
              />
              Unsynced entries when ending the workday
            </label>
            {REMINDERS.map((r) => (
              <div className="settings-reminder" key={r.flag}>
                <label className="settings-check">
                  <input
                    type="checkbox"
                    checked={notifications[r.flag]}
                    onChange={(e) => setNotifications({ ...notifications, [r.flag]: e.target.checked })}
                  />
                  {r.label}
                </label>
                <input
                  type="text"
                  aria-label={`${r.label} (duration)`}
                  value={thresholds[r.minutes]}
                  onChange={(e) => setThresholds({ ...thresholds, [r.minutes]: e.target.value })}
                  disabled={!notifications[r.flag]}
                />
              </div>
            ))}
            <p className="field-hint">
              Durations like 30m, 1h 30m or 2h. A long-running timer is reminded about again
              each time it runs that long once more; the other reminders fire once per
              occurrence. The workday reminder counts time worked today, breaks excluded.
            </p>
          </>
        )}
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
