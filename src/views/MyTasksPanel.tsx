import { useState } from "react";
import type { Task } from "../api/types";

interface MyTasksPanelProps {
  tasks: Task[];
  onRefresh: () => Promise<void>;
  onStartTimer: (taskId: number) => Promise<void>;
}

export function MyTasksPanel({ tasks, onRefresh, onStartTimer }: MyTasksPanelProps) {
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  return (
    <section className="panel">
      <div className="panel-header">
        <h2>My Tasks</h2>
        <button onClick={handleRefresh} disabled={refreshing}>
          {refreshing ? "Refreshing…" : "Refresh"}
        </button>
      </div>
      {error && <p className="error">{error}</p>}
      {tasks.length === 0 ? (
        <p className="empty-hint">No assigned tickets loaded yet — click Refresh.</p>
      ) : (
        <ul className="task-list">
          {tasks.map((t) => (
            <li key={t.id}>
              <span className="task-key">{t.jiraKey}</span>
              <span className="task-summary">{t.summary}</span>
              <button onClick={() => onStartTimer(t.id)}>Start</button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
