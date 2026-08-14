import { FormEvent, useState } from "react";
import { createManualEntry, updateTimeEntry } from "../api/commands";
import type { Task, TimeEntry } from "../api/types";
import { combine, toDateInput, toTimeInput } from "../lib/format";

interface ManualEntryFormProps {
  tasks: Task[];
  entry?: TimeEntry;
  onClose: () => void;
  onSaved: () => Promise<void>;
}

export function ManualEntryForm({ tasks, entry, onClose, onSaved }: ManualEntryFormProps) {
  const now = new Date();
  const [taskId, setTaskId] = useState<number | "">(entry?.taskId ?? "");
  const [date, setDate] = useState(entry ? toDateInput(entry.startedAt) : toDateInput(now.toISOString()));
  const [startTime, setStartTime] = useState(
    entry ? toTimeInput(entry.startedAt) : toTimeInput(now.toISOString()),
  );
  const [mode, setMode] = useState<"duration" | "endTime">(entry ? "endTime" : "duration");
  const [endTime, setEndTime] = useState(entry?.endedAt ? toTimeInput(entry.endedAt) : startTime);
  const [durationMinutes, setDurationMinutes] = useState(
    entry?.durationSeconds ? Math.round(entry.durationSeconds / 60) : 30,
  );
  const [comment, setComment] = useState(entry?.comment ?? "");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    if (taskId === "") return;
    setSaving(true);
    setError(null);
    try {
      const startedAt = combine(date, startTime);
      if (entry) {
        await updateTimeEntry({
          id: entry.id,
          taskId,
          startedAt,
          ...(mode === "endTime"
            ? { endedAt: combine(date, endTime) }
            : { durationSeconds: durationMinutes * 60 }),
          comment,
        });
      } else {
        await createManualEntry({
          taskId,
          startedAt,
          ...(mode === "endTime"
            ? { endedAt: combine(date, endTime) }
            : { durationSeconds: durationMinutes * 60 }),
          comment: comment.trim() || undefined,
        });
      }
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
        <h2>{entry ? "Edit time entry" : "New manual entry"}</h2>
        <label>
          Ticket
          <select value={taskId} onChange={(e) => setTaskId(Number(e.target.value) || "")} required>
            <option value="">Select a ticket…</option>
            {tasks.map((t) => (
              <option key={t.id} value={t.id}>
                {t.jiraKey} — {t.summary}
              </option>
            ))}
          </select>
        </label>
        <label>
          Date
          <input type="date" value={date} onChange={(e) => setDate(e.target.value)} required />
        </label>
        <label>
          Start time
          <input type="time" value={startTime} onChange={(e) => setStartTime(e.target.value)} required />
        </label>
        <div className="mode-toggle">
          <label>
            <input
              type="radio"
              checked={mode === "duration"}
              onChange={() => setMode("duration")}
            />
            Duration
          </label>
          <label>
            <input type="radio" checked={mode === "endTime"} onChange={() => setMode("endTime")} />
            End time
          </label>
        </div>
        {mode === "duration" ? (
          <label>
            Duration (minutes)
            <input
              type="number"
              min={1}
              value={durationMinutes}
              onChange={(e) => setDurationMinutes(Number(e.target.value))}
              required
            />
          </label>
        ) : (
          <label>
            End time
            <input type="time" value={endTime} onChange={(e) => setEndTime(e.target.value)} required />
          </label>
        )}
        <label>
          Comment (optional)
          <input type="text" value={comment} onChange={(e) => setComment(e.target.value)} />
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
