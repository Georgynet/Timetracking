import { openUrl } from "@tauri-apps/plugin-opener";
import { useState } from "react";
import type { Task } from "../api/types";
import { jiraIssueUrl } from "../lib/jira";

interface MyTasksPanelProps {
  tasks: Task[];
  jiraBaseUrl: string;
  /** How many entries to show before the list scrolls (see Settings). */
  rows: number;
  /** Whether the sprint filter starts ticked (see Settings). */
  currentSprintDefault: boolean;
  onRefresh: () => Promise<void>;
  onStartTimer: (taskId: number) => Promise<void>;
}

export function MyTasksPanel({ tasks, jiraBaseUrl, rows, currentSprintDefault, onRefresh, onStartTimer }: MyTasksPanelProps) {
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Seeded from the saved preference, which defaults to on: the current sprint is
  // what's being worked on almost every time, and the full assigned list buries it.
  // Unticking is a per-session look at everything; the preference decides each launch.
  const [currentSprintOnly, setCurrentSprintOnly] = useState(currentSprintDefault);

  async function handleRefresh() {
    setRefreshing(true);
    setError(null);
    try {
      await onRefresh();
    } catch (err) {
      setError(err as string);
    } finally {
      setRefreshing(false);
    }
  }

  const visibleTasks = currentSprintOnly ? tasks.filter((t) => t.isInCurrentSprint) : tasks;

  return (
    <section className="panel">
      <div className="panel-header">
        {/* The count follows the filter — it counts what's actually listed below. */}
        <h2>
          My Tasks <span className="panel-count">({visibleTasks.length})</span>
        </h2>
        <div className="panel-header-actions">
          <label className="sprint-toggle">
            <input
              type="checkbox"
              checked={currentSprintOnly}
              onChange={(e) => setCurrentSprintOnly(e.target.checked)}
            />
            Current sprint
          </label>
          <button onClick={handleRefresh} disabled={refreshing}>
            {refreshing ? "Refreshing…" : "Refresh"}
          </button>
        </div>
      </div>
      {error && <p className="error">{error}</p>}
      {tasks.length === 0 ? (
        <p className="empty-hint">No assigned tickets loaded yet — click Refresh.</p>
      ) : visibleTasks.length === 0 ? (
        <p className="empty-hint">
          No tickets in the current sprint — untick the filter to see everything
          assigned to you.
        </p>
      ) : (
        <ul
          className="task-list task-list-capped"
          style={{ maxHeight: `calc(${rows} * 37px + ${rows - 1} * 0.4em)` }}
        >
          {visibleTasks.map((t) => (
            <li key={t.id}>
              <a
                className="task-key jira-link"
                href={jiraIssueUrl(jiraBaseUrl, t.jiraKey)}
                onClick={(e) => {
                  e.preventDefault();
                  openUrl(jiraIssueUrl(jiraBaseUrl, t.jiraKey));
                }}
              >
                {t.jiraKey}
              </a>
              <span className="task-summary">{t.summary}</span>
              <button onClick={() => onStartTimer(t.id)}>Start</button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
