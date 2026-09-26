import { FormEvent, useState } from "react";
import { updateWorkday } from "../api/commands";
import type { WorkdaySession } from "../api/types";
import { combine, toDateInput, toTimeInput } from "../lib/format";

interface EditWorkdayFormProps {
  session: WorkdaySession;
  onClose: () => void;
  onSaved: () => Promise<void>;
}

export function EditWorkdayForm({ session, onClose, onSaved }: EditWorkdayFormProps) {
  const [startDate, setStartDate] = useState(toDateInput(session.startedAt));
  const [startTime, setStartTime] = useState(toTimeInput(session.startedAt));
  // Independent from `startDate` — a session can start before and end after local
  // midnight (e.g. a forgotten-to-stop workday found the next day), so sharing a single
  // date field between start and end silently produced an end timestamp before the
  // start whenever that happened, which `update_workday` then always rejected.
  const [endDate, setEndDate] = useState(
    session.endedAt ? toDateInput(session.endedAt) : toDateInput(session.startedAt),
  );
  const [endTime, setEndTime] = useState(
    session.endedAt ? toTimeInput(session.endedAt) : toTimeInput(session.startedAt),
  );
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setSaving(true);
    setError(null);
    try {
      await updateWorkday({
        id: session.id,
        startedAt: combine(startDate, startTime),
        endedAt: combine(endDate, endTime),
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
        <h2>Edit workday</h2>
        <label>
          Start date
          <input type="date" value={startDate} onChange={(e) => setStartDate(e.target.value)} required />
        </label>
        <label>
          Start time
          <input type="time" value={startTime} onChange={(e) => setStartTime(e.target.value)} required />
        </label>
        <label>
          End date
          <input type="date" value={endDate} onChange={(e) => setEndDate(e.target.value)} required />
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
