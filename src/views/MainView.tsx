import { useEffect, useState } from "react";
import { isTrayAvailable } from "../api/commands";
import type { SyncReport } from "../api/types";
import { useStore } from "../state/store";
import { FavoritesPanel } from "./FavoritesPanel";
import { HeaderBar } from "./HeaderBar";
import { HistoryList } from "./HistoryList";
import { MyTasksPanel } from "./MyTasksPanel";
import { SyncReportModal } from "./SyncReportModal";
import { TimerWidget } from "./TimerWidget";
import { WorkdayWidget } from "./WorkdayWidget";

export function MainView({ onReconfigure }: { onReconfigure: () => void }) {
  const {
    settings,
    myTasks,
    favoriteTasks,
    activeTimer,
    unsyncedCount,
    activeWorkday,
    dailySummary,
    weekSummary,
    monthSummary,
    loadTasks,
    refreshMyTasks,
    loadActiveTimer,
    loadUnsyncedCount,
    startTimer,
    stopTimer,
    runSync,
    loadActiveWorkday,
    loadPeriodSummaries,
    startWorkday,
    endWorkday,
    startBreak,
    endBreak,
  } = useStore();
  const [trayAvailable, setTrayAvailable] = useState(true);
  const [syncReport, setSyncReport] = useState<SyncReport | null>(null);
  const [historyRefreshSignal, setHistoryRefreshSignal] = useState(0);

  useEffect(() => {
    loadTasks();
    loadActiveTimer();
    loadUnsyncedCount();
    loadActiveWorkday();
    loadPeriodSummaries();
    isTrayAvailable().then(setTrayAvailable);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // A sync can change the "logged" side of the worked-vs-logged comparisons (a
  // worklog push can succeed or fail), so refresh them alongside the history list.
  // Direct edits/deletes in History go through `handleEntriesChanged` instead.
  useEffect(() => {
    if (historyRefreshSignal > 0) loadPeriodSummaries();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [historyRefreshSignal]);

  async function handleStartTimer(taskId: number) {
    await startTimer(taskId);
    setHistoryRefreshSignal((n) => n + 1);
  }

  async function handleStopTimer() {
    await stopTimer();
    setHistoryRefreshSignal((n) => n + 1);
  }

  async function handleSync(): Promise<SyncReport> {
    const report = await runSync();
    setSyncReport(report);
    setHistoryRefreshSignal((n) => n + 1);
    return report;
  }

  // The running timer can never be edited/deleted through History (see HistoryList),
  // but refresh it here too whenever an entry changes — cheap insurance against the
  // timer widget ever showing state that's stale relative to the DB. An edit/delete
  // also changes the "logged" side of the worked-vs-logged comparisons, so refresh
  // those too — this is the only path into HistoryList's edit/delete that doesn't
  // already go through `historyRefreshSignal`.
  async function handleEntriesChanged() {
    await Promise.all([loadActiveTimer(), loadUnsyncedCount(), loadPeriodSummaries()]);
  }

  if (!settings || !settings.jiraBaseUrl) return null;

  // Favorites and "my tasks" can overlap (e.g. a favorited ticket that's also
  // assigned to you) — de-dupe by id for the timer's ticket picker and history form.
  const allTasks = [...myTasks, ...favoriteTasks.filter((f) => !myTasks.some((m) => m.id === f.id))];

  return (
    <div className="main-view">
      <HeaderBar
        settings={settings}
        unsyncedCount={unsyncedCount}
        trayAvailable={trayAvailable}
        onSync={handleSync}
        onReconfigure={onReconfigure}
      />
      <WorkdayWidget
        activeWorkday={activeWorkday}
        dailySummary={dailySummary}
        weekSummary={weekSummary}
        monthSummary={monthSummary}
        onStartWorkday={startWorkday}
        onEndWorkday={endWorkday}
        onStartBreak={startBreak}
        onEndBreak={endBreak}
      />
      <TimerWidget
        activeTimer={activeTimer}
        tasks={allTasks}
        onStart={handleStartTimer}
        onStop={handleStopTimer}
      />
      <div className="panels-row">
        <MyTasksPanel
          tasks={myTasks}
          jiraBaseUrl={settings.jiraBaseUrl}
          onRefresh={refreshMyTasks}
          onStartTimer={handleStartTimer}
        />
        <FavoritesPanel
          tasks={favoriteTasks}
          jiraBaseUrl={settings.jiraBaseUrl}
          onChanged={loadTasks}
          onStartTimer={handleStartTimer}
        />
      </div>
      <HistoryList
        tasks={allTasks}
        jiraBaseUrl={settings.jiraBaseUrl}
        refreshSignal={historyRefreshSignal}
        onEntriesChanged={handleEntriesChanged}
      />
      {syncReport && <SyncReportModal report={syncReport} onClose={() => setSyncReport(null)} />}
    </div>
  );
}
