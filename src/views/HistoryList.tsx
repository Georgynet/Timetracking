import { confirm } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { addDays, endOfDay, format, isToday, isYesterday, startOfDay, subDays } from "date-fns";
import { useEffect, useState } from "react";
import { deleteDraftEntry, listTimeEntries } from "../api/commands";
import type { Task, TimeEntry } from "../api/types";
import { formatDuration } from "../lib/format";
import { jiraIssueUrl } from "../lib/jira";
import { ManualEntryForm } from "./ManualEntryForm";

/** A local calendar day's bounds, as ISO instants, for filtering history to that day. */
function dayRange(date: Date): { from: string; to: string } {
  return { from: startOfDay(date).toISOString(), to: endOfDay(date).toISOString() };
}

function dayLabel(date: Date): string {
  if (isToday(date)) return "Today";
  if (isYesterday(date)) return "Yesterday";
  return format(date, "EEE, MMM d, yyyy");
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
  const [selectedDate, setSelectedDate] = useState(() => startOfDay(new Date()));
  const [entries, setEntries] = useState<TimeEntry[]>([]);
  const [editingEntry, setEditingEntry] = useState<TimeEntry | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);

  async function reload() {
    try {
      setEntries(await listTimeEntries(dayRange(selectedDate)));
    } catch (err) {
      setError(err as string);
    }
  }

  useEffect(() => {
    reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshSignal, selectedDate]);

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

  // Sums the Duration column as shown: a still-running entry contributes nothing,
  // because its duration isn't decided yet (`durationSeconds` is null until it stops).
  const totalSeconds = entries.reduce((total, entry) => total + (entry.durationSeconds ?? 0), 0);

  return (
    <section className="panel history-panel">
      <div className="panel-header">
        <h2>History</h2>
        <button onClick={() => setEditingEntry(null)}>New manual entry</button>
      </div>
      <div className="history-nav">
        <button onClick={() => setSelectedDate((d) => subDays(d, 1))}>&lt; Prev</button>
        <span className="history-nav-date">{dayLabel(selectedDate)}</span>
        <button onClick={() => setSelectedDate((d) => addDays(d, 1))} disabled={isToday(selectedDate)}>
          Next &gt;
        </button>
      </div>
      {error && <p className="error">{error}</p>}
      {entries.length === 0 ? (
        <p className="empty-hint">
          No time entries {isToday(selectedDate) ? "yet today" : "on this day"}.
        </p>
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
          {/* Counts every row shown for the day, the running entry included — it's a
              row in the table like any other. The total sits under Duration and adds
              up the same column: a running entry has no duration yet, so it counts
              towards Entries but not towards the sum. */}
          <tfoot>
            <tr>
              <th scope="row">Entries</th>
              <td>{entries.length}</td>
              <td>{formatDuration(totalSeconds)}</td>
              <td colSpan={3}></td>
            </tr>
          </tfoot>
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
