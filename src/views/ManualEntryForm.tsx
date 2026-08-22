import { FormEvent, useState } from "react";
import { createManualEntry, resolveTaskByKey, searchJiraIssues, updateTimeEntry } from "../api/commands";
import type { JiraIssue, Task, TimeEntry } from "../api/types";
import { combine, toDateInput, toTimeInput } from "../lib/format";

interface ManualEntryFormProps {
  tasks: Task[];
  entry?: TimeEntry;
  onClose: () => void;
  onSaved: () => Promise<void>;
}

/** The entry being edited may reference a one-off ticket (see `resolveTaskByKey`) that
 * never made it into My Tasks/Favorites, so it wouldn't otherwise appear in `tasks`. */
function taskFromEntry(entry: TimeEntry): Task {
  return {
    id: entry.taskId,
    jiraKey: entry.taskKey,
    summary: entry.taskSummary,
    isFavorite: false,
    isAssignedToMe: false,
    isInCurrentSprint: false,
    lastSyncedAt: null,
    // A one-off ticket resolved for this entry: whatever it was last tracked at is
    // irrelevant here, the dialog only needs it to render in the picker.
    lastTrackedAt: null,
  };
}

export function ManualEntryForm({ tasks, entry, onClose, onSaved }: ManualEntryFormProps) {
  const now = new Date();
  // Tickets resolved via the in-form search (or the edited entry's own ticket, if it's
  // a one-off not already in `tasks`) — merged into the dropdown alongside `tasks`.
  const [foundTasks, setFoundTasks] = useState<Task[]>(() =>
    entry && !tasks.some((t) => t.id === entry.taskId) ? [taskFromEntry(entry)] : [],
  );
  const [taskId, setTaskId] = useState<number | "">(entry?.taskId ?? "");
  const [showSearch, setShowSearch] = useState(false);
  const [query, setQuery] = useState("");
  const [searchResults, setSearchResults] = useState<JiraIssue[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
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

  async function handleSearch() {
    if (!query.trim()) return;
    setSearching(true);
    setSearchError(null);
    try {
      setSearchResults(await searchJiraIssues(query.trim()));
    } catch (err) {
      setSearchError(err as string);
      setSearchResults([]);
    } finally {
      setSearching(false);
    }
  }

  async function handleAddTicket(key: string) {
    setSearchError(null);
    try {
      const task = await resolveTaskByKey(key);
      setFoundTasks((prev) => [...prev.filter((t) => t.id !== task.id), task]);
      setTaskId(task.id);
      setShowSearch(false);
      setQuery("");
      setSearchResults([]);
    } catch (err) {
      setSearchError(err as string);
    }
  }

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
            {[...tasks, ...foundTasks.filter((f) => !tasks.some((t) => t.id === f.id))].map((t) => (
              <option key={t.id} value={t.id}>
                {t.jiraKey} — {t.summary}
              </option>
            ))}
          </select>
        </label>
        {showSearch ? (
          <div className="ticket-search">
            <div className="favorite-search">
              <input
                type="text"
                placeholder="Search by key (TEAM-1) or free text in Jira…"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && (e.preventDefault(), handleSearch())}
                autoFocus
              />
              <button type="button" onClick={handleSearch} disabled={searching || !query.trim()}>
                {searching ? "Searching…" : "Search"}
              </button>
              <button
                type="button"
                className="link-button"
                onClick={() => {
                  setShowSearch(false);
                  setQuery("");
                  setSearchResults([]);
                  setSearchError(null);
                }}
              >
                Cancel
              </button>
            </div>
            {searchError && <p className="error">{searchError}</p>}
            {searchResults.length > 0 && (
              <ul className="task-list task-list-capped">
                {searchResults.map((issue) => (
                  <li key={issue.key}>
                    <span className="task-key">{issue.key}</span>
                    <span className="task-summary">{issue.summary}</span>
                    <button type="button" onClick={() => handleAddTicket(issue.key)}>
                      Use
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        ) : (
          <button type="button" className="link-button" onClick={() => setShowSearch(true)}>
            Ticket not in the list? Search Jira for a one-off ticket…
          </button>
        )}
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
