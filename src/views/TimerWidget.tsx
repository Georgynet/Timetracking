import { useEffect, useRef, useState } from "react";
import { resolveTaskByKey, updateRunningComment } from "../api/commands";
import type { ActiveTimer, Task } from "../api/types";
import { formatElapsed } from "../lib/format";
import { extractIssueKey } from "../lib/jira";

/** Starting by key is mostly for reviewing someone else's ticket (ADR-0040), so the
 * comment starts out as this — editable, and dropped again if the form is cancelled. */
const BY_KEY_COMMENT = "CR";

interface TimerWidgetProps {
  activeTimer: ActiveTimer | null;
  tasks: Task[];
  onStart: (taskId: number, comment?: string) => Promise<void>;
  onStop: () => Promise<void>;
  /** The tray's "Ticket key…" item was used: open and focus the key field, then ack. */
  startByKeyRequested: boolean;
  onStartByKeyHandled: () => void;
}

export function TimerWidget({
  activeTimer,
  tasks,
  onStart,
  onStop,
  startByKeyRequested,
  onStartByKeyHandled,
}: TimerWidgetProps) {
  const [now, setNow] = useState(() => Date.now());
  const [selectedTaskId, setSelectedTaskId] = useState<number | "">("");
  const [comment, setComment] = useState("");
  // Start (or switch to) a ticket by typing its key — for tickets that aren't in My
  // Tasks/Favorites, typically a review of someone else's work.
  const [byKey, setByKey] = useState(false);
  const [keyInput, setKeyInput] = useState("");
  const keyInputRef = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // The note on the running entry. It lives on the entry itself (as its comment), so
  // it's saved while typing rather than on Stop — the timer can also be stopped from
  // the tray, or by switching tickets, and the note must survive that too.
  const [note, setNote] = useState("");
  const savedNote = useRef("");
  const saveTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);
  const runningId = activeTimer?.id ?? null;

  useEffect(() => {
    if (!activeTimer) return;
    const interval = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(interval);
  }, [activeTimer]);

  // A different entry (started here, from the tray, or after a switch) brings its own
  // note. Keyed on the id only, so a refetch of the same timer doesn't clobber typing.
  useEffect(() => {
    const initial = activeTimer?.comment ?? "";
    setNote(initial);
    savedNote.current = initial;
    return () => {
      if (saveTimeout.current) clearTimeout(saveTimeout.current);
      saveTimeout.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runningId]);

  async function saveNote(value: string) {
    if (saveTimeout.current) clearTimeout(saveTimeout.current);
    saveTimeout.current = null;
    if (runningId === null || value.trim() === savedNote.current.trim()) return;
    try {
      await updateRunningComment(runningId, value.trim());
      savedNote.current = value;
    } catch (err) {
      setError(err as string);
    }
  }

  function handleNoteChange(value: string) {
    setNote(value);
    if (saveTimeout.current) clearTimeout(saveTimeout.current);
    saveTimeout.current = setTimeout(() => saveNote(value), 800);
  }

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

  function openByKey() {
    setByKey(true);
    setComment((c) => (c.trim() ? c : BY_KEY_COMMENT));
    keyInputRef.current?.focus();
  }

  function closeByKey() {
    setByKey(false);
    setKeyInput("");
    setComment((c) => (c === BY_KEY_COMMENT ? "" : c));
  }

  useEffect(() => {
    if (!startByKeyRequested) return;
    openByKey();
    onStartByKeyHandled();
    // The field may only mount with this render, after which `autoFocus` covers it.
    requestAnimationFrame(() => keyInputRef.current?.focus());
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [startByKeyRequested]);

  async function handleStartByKey() {
    const key = extractIssueKey(keyInput);
    if (!key) {
      setError("Enter a ticket key like PROJ-123 or paste a Jira link");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      // Fetches the ticket from Jira (so a typo fails here, not at sync time) and
      // stores it locally; it then shows up under Recent/Resume in the tray.
      const task = await resolveTaskByKey(key);
      // Switching tickets stops the running one, so its note must be saved first.
      if (activeTimer) await saveNote(note);
      await onStart(task.id, comment.trim() || undefined);
      setByKey(false);
      setKeyInput("");
      setComment("");
    } catch (err) {
      setError(err as string);
    } finally {
      setBusy(false);
    }
  }

  const byKeyFields = (
    <>
      <input
        ref={keyInputRef}
        className="timer-key"
        type="text"
        placeholder="PROJ-123 or Jira link"
        value={keyInput}
        autoFocus
        onChange={(e) => setKeyInput(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") handleStartByKey();
          if (e.key === "Escape") closeByKey();
        }}
      />
      <input
        className="timer-start-comment"
        type="text"
        placeholder="Comment (optional)"
        value={comment}
        onChange={(e) => setComment(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") handleStartByKey();
          if (e.key === "Escape") closeByKey();
        }}
      />
      <button onClick={handleStartByKey} disabled={busy || !keyInput.trim()}>
        {activeTimer ? "Switch" : "Start"}
      </button>
      <button type="button" className="link-button" onClick={closeByKey}>
        Cancel
      </button>
    </>
  );

  async function handleStop() {
    setBusy(true);
    setError(null);
    try {
      await saveNote(note);
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
        <input
          className="timer-note"
          type="text"
          placeholder="Comment (optional)"
          value={note}
          onChange={(e) => handleNoteChange(e.target.value)}
          onBlur={() => saveNote(note)}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
          }}
        />
        {!byKey && (
          <button type="button" className="link-button" onClick={openByKey}>
            Switch to key…
          </button>
        )}
        <button onClick={handleStop} disabled={busy}>
          Stop
        </button>
        {byKey && <div className="timer-key-row">{byKeyFields}</div>}
      </section>
    );
  }

  if (byKey) {
    return (
      <section className="timer-widget">
        {error && <p className="error">{error}</p>}
        {byKeyFields}
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
        className="timer-start-comment"
        type="text"
        placeholder="Comment (optional)"
        value={comment}
        onChange={(e) => setComment(e.target.value)}
      />
      <button onClick={handleStart} disabled={busy || selectedTaskId === ""}>
        Start
      </button>
      <button type="button" className="link-button" onClick={openByKey}>
        By key…
      </button>
    </section>
  );
}
