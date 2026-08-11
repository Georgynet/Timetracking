import { confirm } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { deleteDraftEntry, listTimeEntries } from "../api/commands";
import type { Task, TimeEntry } from "../api/types";
import { jiraIssueUrl } from "../lib/jira";
import { ManualEntryForm } from "./ManualEntryForm";

function formatDuration(seconds: number | null): string {
  if (seconds === null) return "—";
  const h = Math.floor(seconds / 3600);
  const m = Math.round((seconds % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

function canEdit(entry: TimeEntry): boolean {
  return entry.endedAt !== null && !entry.isSynced;
}

function canDelete(entry: TimeEntry): boolean {
  return entry.endedAt !== null && !entry.isSynced && entry.jiraWorklogId === null;
}

interface HistoryListProps {
  tasks: Task[];
  jiraBaseUrl: string;
  refreshSignal: number;
  onEntriesChanged: () => Promise<void>;
}

export function HistoryList({ tasks, jiraBaseUrl, refreshSignal, onEntriesChanged }: HistoryListProps) {
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
    const confirmed = await confirm("Delete this entry? This cannot be undone.", {
      title: "Delete time entry",
      kind: "warning",
    });
    if (!confirmed) return;
    try {
      await deleteDraftEntry(id);
      await reload();
      await onEntriesChanged();
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
                <td>
                  <a
                    className="jira-link"
                    title={entry.taskSummary}
                    href={jiraIssueUrl(jiraBaseUrl, entry.taskKey)}
                    onClick={(e) => {
                      e.preventDefault();
                      openUrl(jiraIssueUrl(jiraBaseUrl, entry.taskKey));
                    }}
                  >
                    {entry.taskKey}
                  </a>
                </td>
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
                  {canEdit(entry) && (
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
