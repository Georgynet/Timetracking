import { FormEvent, useState } from "react";
import type { Preferences } from "../api/types";

interface SettingsModalProps {
  preferences: Preferences;
  onClose: () => void;
  onSave: (next: Preferences) => Promise<void>;
}

/**
 * App preferences. Currently just how tall the two task panels are; the shape is
 * built to grow, since the backing store is a key/value table rather than columns
 * (see ADR-0025).
 */
export function SettingsModal({ preferences, onClose, onSave }: SettingsModalProps) {
  const [myTasksRows, setMyTasksRows] = useState(preferences.myTasksRows);
  const [favoritesRows, setFavoritesRows] = useState(preferences.favoritesRows);
  const [currentSprintDefault, setCurrentSprintDefault] = useState(preferences.currentSprintDefault);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setSaving(true);
    setError(null);
    try {
      await onSave({ myTasksRows, favoritesRows, currentSprintDefault });
      onClose();
    } catch (err) {
      setError(err as string);
    } finally {
      setSaving(false);
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
