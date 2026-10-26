import { confirm } from "@tauri-apps/plugin-dialog";
import { FormEvent, useState } from "react";
import { deleteBreak, updateBreak } from "../api/commands";
import type { WorkBreak } from "../api/types";
import { combine, toDateInput, toTimeInput } from "../lib/format";
import { useBackdropClose } from "../lib/modal";

interface EditBreakFormProps {
  brk: WorkBreak;
  onClose: () => void;
  onSaved: () => Promise<void>;
}

export function EditBreakForm({ brk, onClose, onSaved }: EditBreakFormProps) {
  const [startDate, setStartDate] = useState(toDateInput(brk.startedAt));
  const [startTime, setStartTime] = useState(toTimeInput(brk.startedAt));
  // Independent from `startDate` — the parent workday can span local midnight, so a
  // break within it can too; sharing one date field between start and end silently
  // produced an end timestamp before the start whenever that happened.
  const [endDate, setEndDate] = useState(brk.endedAt ? toDateInput(brk.endedAt) : toDateInput(brk.startedAt));
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

  async function handleDelete() {
    const confirmed = await confirm("Delete this break? This cannot be undone.", {
      title: "Delete break",
      kind: "warning",
    });
    if (!confirmed) return;
    setSaving(true);
    setError(null);
    try {
      await deleteBreak(brk.id);
      await onSaved();
      onClose();
    } catch (err) {
      setError(err as string);
    } finally {
      setSaving(false);
    }
  }

  const backdrop = useBackdropClose(onClose);

  return (
    <div className="modal-backdrop" {...backdrop}>
      <form className="modal" onSubmit={handleSubmit}>
        <h2>Edit break</h2>
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
          <button type="button" className="link-button" onClick={handleDelete} disabled={saving}>
            Delete
          </button>
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
