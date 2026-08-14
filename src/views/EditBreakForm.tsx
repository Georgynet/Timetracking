import { FormEvent, useState } from "react";
import { updateBreak } from "../api/commands";
import type { WorkBreak } from "../api/types";
import { combine, toDateInput, toTimeInput } from "../lib/format";

interface EditBreakFormProps {
  brk: WorkBreak;
  onClose: () => void;
  onSaved: () => Promise<void>;
}

export function EditBreakForm({ brk, onClose, onSaved }: EditBreakFormProps) {
  const [date, setDate] = useState(toDateInput(brk.startedAt));
  const [startTime, setStartTime] = useState(toTimeInput(brk.startedAt));
  const [endTime, setEndTime] = useState(brk.endedAt ? toTimeInput(brk.endedAt) : toTimeInput(brk.startedAt));
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setSaving(true);
    setError(null);
    try {
      await updateBreak({
        id: brk.id,
        startedAt: combine(date, startTime),
        endedAt: combine(date, endTime),
      });
      await onSaved();
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
        <h2>Edit break</h2>
        <label>
          Date
          <input type="date" value={date} onChange={(e) => setDate(e.target.value)} required />
        </label>
        <label>
          Start time
          <input type="time" value={startTime} onChange={(e) => setStartTime(e.target.value)} required />
        </label>
        <label>
          End time
          <input type="time" value={endTime} onChange={(e) => setEndTime(e.target.value)} required />
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
