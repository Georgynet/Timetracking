import { useEffect, useState } from "react";
import type { DailySummary, RangeSummary, WorkBreak, WorkdaySession, WorkdayStatus } from "../api/types";
import { formatDuration, formatElapsed, localDateString, shiftDate, toTimeInput } from "../lib/format";
import { EditBreakForm } from "./EditBreakForm";
import { EditWorkdayForm } from "./EditWorkdayForm";

interface WorkdayWidgetProps {
  activeWorkday: WorkdayStatus | null;
  /** Completed sessions for `workdaySessionsDate` (see ADR-0032) — not necessarily today. */
  workdaySessions: WorkdaySession[];
  workdaySessionsDate: string;
  /** Preference gate for the two rows this renders — off by default, since they cost
   *  permanent vertical space for a need that's rare (see ADR-0032's Settings toggle). */
  workdayEditingEnabled: boolean;
  dailySummary: DailySummary | null;
  weekSummary: RangeSummary | null;
  monthSummary: RangeSummary | null;
  onStartWorkday: () => Promise<void>;
  onEndWorkday: () => Promise<void>;
  onStartBreak: () => Promise<void>;
  onEndBreak: () => Promise<void>;
  onBreakUpdated: () => Promise<void>;
  onWorkdayUpdated: () => Promise<void>;
  onNavigateDate: (date: string) => Promise<void>;
}

function breakSeconds(brk: WorkBreak, nowMs: number): number {
  const start = new Date(brk.startedAt).getTime();
  const end = brk.endedAt ? new Date(brk.endedAt).getTime() : nowMs;
  return Math.max(0, (end - start) / 1000);
}

/** The current session's own worked time — span since it started, minus its breaks. */
function currentSessionWorkedSeconds(workday: WorkdayStatus, nowMs: number): number {
  const start = new Date(workday.startedAt).getTime();
  const end = workday.endedAt ? new Date(workday.endedAt).getTime() : nowMs;
  const span = Math.max(0, (end - start) / 1000);
  const breaks = workday.breaks.reduce((total, brk) => total + breakSeconds(brk, nowMs), 0);
  return Math.max(0, span - breaks);
}

function currentSessionBreakSeconds(workday: WorkdayStatus, nowMs: number): number {
  return workday.breaks.reduce((total, brk) => total + breakSeconds(brk, nowMs), 0);
}

/** A completed session's own worked time — span minus its breaks, same formula as
 *  `currentSessionWorkedSeconds` but for a session that's already ended. */
function sessionWorkedSeconds(session: WorkdaySession, nowMs: number): number {
  const start = new Date(session.startedAt).getTime();
  const end = session.endedAt ? new Date(session.endedAt).getTime() : nowMs;
  const span = Math.max(0, (end - start) / 1000);
  const breaks = session.breaks.reduce((total, brk) => total + breakSeconds(brk, nowMs), 0);
  return Math.max(0, span - breaks);
}

function formatDiff(diffSeconds: number): string {
  const sign = diffSeconds > 0 ? "+" : diffSeconds < 0 ? "−" : "";
  return `${sign}${formatDuration(Math.abs(diffSeconds))}`;
}

export function WorkdayWidget({
  activeWorkday,
  workdaySessions,
  workdaySessionsDate,
  workdayEditingEnabled,
  dailySummary,
  weekSummary,
  monthSummary,
  onStartWorkday,
  onEndWorkday,
  onStartBreak,
  onEndBreak,
  onBreakUpdated,
  onWorkdayUpdated,
  onNavigateDate,
}: WorkdayWidgetProps) {
  const [now, setNow] = useState(() => Date.now());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editingBreak, setEditingBreak] = useState<WorkBreak | null>(null);
  const [editingSession, setEditingSession] = useState<WorkdaySession | null>(null);

  const today = localDateString();
  const endedSessions = workdaySessions.filter((s) => s.endedAt);

  useEffect(() => {
    if (!activeWorkday) return;
    const interval = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(interval);
  }, [activeWorkday]);

  async function guard(action: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (err) {
      setError(err as string);
    } finally {
      setBusy(false);
    }
  }

  // Today's running total resumes across a stop/restart on the same day: it's the
  // already-banked time from any earlier session today plus the current session's own
  // live-ticking elapsed, rather than resetting to zero each time a workday restarts.
  const workedToday = activeWorkday
    ? activeWorkday.priorWorkedSecondsToday + currentSessionWorkedSeconds(activeWorkday, now)
    : (dailySummary?.workedSeconds ?? 0);
  const breaksToday = activeWorkday
    ? activeWorkday.priorBreakSecondsToday + currentSessionBreakSeconds(activeWorkday, now)
    : 0;
  const loggedToday = dailySummary?.loggedSeconds ?? 0;

  // Week/month totals are only as fresh as their last fetch, but they both include
  // today's contribution as of that same fetch (see `loadPeriodSummaries`) — swapping
  // that stale slice for the live-ticking `workedToday` keeps them from visibly lagging
  // behind the "Today" line while a workday is running.
  const workedThisWeek = weekSummary
    ? weekSummary.workedSeconds - (dailySummary?.workedSeconds ?? 0) + workedToday
    : 0;
  const workedThisMonth = monthSummary
    ? monthSummary.workedSeconds - (dailySummary?.workedSeconds ?? 0) + workedToday
    : 0;

  return (
    <section className="workday-widget">
      {error && <p className="error">{error}</p>}
      <div className="workday-row">
        {activeWorkday ? (
          <>
            <div className="workday-display">
              <span className="workday-label">{activeWorkday.isOnBreak ? "On break" : "Workday"}</span>
              <span className="workday-elapsed">{formatElapsed(workedToday)}</span>
            </div>
            <div className="workday-actions">
              {activeWorkday.isOnBreak ? (
                <button onClick={() => guard(onEndBreak)} disabled={busy}>
                  End Break
                </button>
              ) : (
                <button onClick={() => guard(onStartBreak)} disabled={busy}>
                  Start Break
                </button>
              )}
              <button onClick={() => guard(onEndWorkday)} disabled={busy}>
                End Workday
              </button>
            </div>
          </>
        ) : (
          <button onClick={() => guard(onStartWorkday)} disabled={busy}>
            Start Workday
          </button>
        )}
      </div>
      {breaksToday > 0 && (
        <p className="workday-breaks">
          Breaks:{" "}
          {activeWorkday && activeWorkday.breaks.length > 0 ? (
            <>
              {activeWorkday.breaks.map((b, i) => (
                <span key={b.id}>
                  {i > 0 && ", "}
                  {b.endedAt ? (
                    <button type="button" className="link-button" onClick={() => setEditingBreak(b)}>
                      {formatDuration(Math.round(breakSeconds(b, now)))}
                    </button>
                  ) : (
                    formatDuration(Math.round(breakSeconds(b, now)))
                  )}
                </span>
              ))}{" "}
              ({formatDuration(breaksToday)} total today)
            </>
          ) : (
            `${formatDuration(breaksToday)} today`
          )}
        </p>
      )}
      {editingBreak && (
        <EditBreakForm brk={editingBreak} onClose={() => setEditingBreak(null)} onSaved={onBreakUpdated} />
      )}
      {workdayEditingEnabled && (
        <div className="workday-sessions">
          <p className="workday-sessions-nav">
            <button
              type="button"
              className="link-button"
              onClick={() => onNavigateDate(shiftDate(workdaySessionsDate, -1))}
            >
              ‹ Prev day
            </button>
            <span>{workdaySessionsDate === today ? "Today's sessions" : workdaySessionsDate}</span>
            <button
              type="button"
              className="link-button"
              disabled={workdaySessionsDate >= today}
              onClick={() => onNavigateDate(shiftDate(workdaySessionsDate, 1))}
            >
              Next day ›
            </button>
          </p>
          {endedSessions.length === 0 ? (
            <p className="workday-sessions-empty">No completed sessions</p>
          ) : (
            <p>
              {endedSessions.map((s, i) => (
                <span key={s.id}>
                  {i > 0 && ", "}
                  <button type="button" className="link-button" onClick={() => setEditingSession(s)}>
                    {toTimeInput(s.startedAt)}–{toTimeInput(s.endedAt!)} (
                    {formatDuration(Math.round(sessionWorkedSeconds(s, now)))})
                  </button>
                </span>
              ))}
            </p>
          )}
        </div>
      )}
      {editingSession && (
        <EditWorkdayForm
          session={editingSession}
          onClose={() => setEditingSession(null)}
          onSaved={onWorkdayUpdated}
        />
      )}
      {(activeWorkday || dailySummary) && (
        <p className="workday-summary">
          Today: worked {formatDuration(workedToday)} · logged {formatDuration(loggedToday)} · diff{" "}
          {formatDiff(workedToday - loggedToday)}
          {weekSummary && <> · This week: worked {formatDuration(workedThisWeek)}</>}
          {monthSummary && <> · This month: worked {formatDuration(workedThisMonth)}</>}
        </p>
      )}
    </section>
  );
}
