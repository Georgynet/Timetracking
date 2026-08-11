import { useEffect, useState } from "react";
import { deleteDraftEntry, listTimeEntries } from "../api/commands";
import type { Task, TimeEntry } from "../api/types";
import { ManualEntryForm } from "./ManualEntryForm";

function formatDuration(seconds: number | null): string {
  if (seconds === null) return "—";
  const h = Math.floor(seconds / 3600);
  const m = Math.round((seconds % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

function canDelete(entry: TimeEntry): boolean {
  return entry.createdManually && !entry.isSynced && entry.jiraWorklogId === null;
}

interface HistoryListProps {
  tasks: Task[];
  refreshSignal: number;
  onEntriesChanged: () => Promise<void>;
}

export function HistoryList({ tasks, refreshSignal, onEntriesChanged }: HistoryListProps) {
  const [entries, setEntries] = useState<TimeEntry[]>([]);
  const [editingEntry, setEditingEntry] = useState<TimeEntry | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);

  async function reload() {
    try {
      setEntries(await listTimeEntries());
    } catch (err) {
      setError(err as string);
    }
  }

  useEffect(() => {
    reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshSignal]);

  async function handleDelete(id: number) {
    if (!confirm("Delete this entry? This cannot be undone.")) return;
    try {
      await deleteDraftEntry(id);
      await reload();
    } catch (err) {
      setError(err as string);
    }
  }

  async function handleSaved() {
    await reload();
    await onEntriesChanged();
  }

  return (
    <section className="panel history-panel">
      <div className="panel-header">
        <h2>History</h2>
        <button onClick={() => setEditingEntry(null)}>New manual entry</button>
      </div>
      {error && <p className="error">{error}</p>}
      {entries.length === 0 ? (
        <p className="empty-hint">No time entries yet.</p>
      ) : (
        <table className="history-table">
          <thead>
            <tr>
              <th>Ticket</th>
              <th>Started</th>
              <th>Duration</th>
              <th>Comment</th>
              <th>Status</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {entries.map((entry) => (
              <tr key={entry.id}>
                <td>{entry.taskKey}</td>
                <td>{new Date(entry.startedAt).toLocaleString()}</td>
                <td>{formatDuration(entry.durationSeconds)}</td>
                <td className="comment-cell">{entry.comment ?? ""}</td>
                <td>
                  <span
                    className={
                      entry.endedAt === null
                        ? "badge badge-running"
                        : entry.isSynced
                          ? "badge badge-synced"
                          : "badge badge-pending"
                    }
                  >
                    {entry.endedAt === null ? "Running" : entry.isSynced ? "Synced" : "Pending"}
                  </span>
                </td>
                <td className="row-actions">
                  {entry.endedAt !== null && (
                    <button className="link-button" onClick={() => setEditingEntry(entry)}>
                      Edit
                    </button>
                  )}
                  {canDelete(entry) && (
                    <button className="link-button" onClick={() => handleDelete(entry.id)}>
                      Delete
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {editingEntry !== undefined && (
        <ManualEntryForm
          tasks={tasks}
          entry={editingEntry ?? undefined}
          onClose={() => setEditingEntry(undefined)}
          onSaved={handleSaved}
        />
      )}
    </section>
  );
}
