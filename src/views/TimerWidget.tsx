import { useEffect, useState } from "react";
import type { ActiveTimer, Task } from "../api/types";
import { formatElapsed } from "../lib/format";

interface TimerWidgetProps {
  activeTimer: ActiveTimer | null;
  tasks: Task[];
  onStart: (taskId: number, comment?: string) => Promise<void>;
  onStop: () => Promise<void>;
}

export function TimerWidget({ activeTimer, tasks, onStart, onStop }: TimerWidgetProps) {
  const [now, setNow] = useState(() => Date.now());
  const [selectedTaskId, setSelectedTaskId] = useState<number | "">("");
  const [comment, setComment] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!activeTimer) return;
    const interval = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(interval);
  }, [activeTimer]);

  async function handleStart() {
    if (selectedTaskId === "") return;
    setBusy(true);
    setError(null);
    try {
      await onStart(selectedTaskId, comment.trim() || undefined);
      setComment("");
    } catch (err) {
      setError(err as string);
    } finally {
      setBusy(false);
    }
  }

  async function handleStop() {
    setBusy(true);
    setError(null);
    try {
      await onStop();
    } catch (err) {
      setError(err as string);
    } finally {
      setBusy(false);
    }
  }

  if (activeTimer) {
    const elapsedSeconds = Math.max(0, (now - new Date(activeTimer.startedAt).getTime()) / 1000);
    return (
      <section className="timer-widget timer-running">
        {error && <p className="error">{error}</p>}
        {activeTimer.isStale && (
          <p className="stale-banner">
            This timer has been running a long time — still working on it, or forgot to
            stop it?
          </p>
        )}
        <div className="timer-display">
          <span className="timer-task">{activeTimer.taskKey}</span>
          <span className="timer-elapsed">{formatElapsed(elapsedSeconds)}</span>
        </div>
        <button onClick={handleStop} disabled={busy}>
          Stop
        </button>
      </section>
    );
  }

  return (
    <section className="timer-widget">
      {error && <p className="error">{error}</p>}
      <select value={selectedTaskId} onChange={(e) => setSelectedTaskId(Number(e.target.value) || "")}>
        <option value="">Select a ticket…</option>
        {tasks.map((t) => (
          <option key={t.id} value={t.id}>
            {t.jiraKey} — {t.summary}
          </option>
        ))}
      </select>
      <input
        type="text"
        placeholder="Comment (optional)"
        value={comment}
        onChange={(e) => setComment(e.target.value)}
      />
      <button onClick={handleStart} disabled={busy || selectedTaskId === ""}>
        Start
      </button>
    </section>
  );
}
