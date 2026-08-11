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

export function MainView({ onReconfigure }: { onReconfigure: () => void }) {
  const {
    settings,
    myTasks,
    favoriteTasks,
    activeTimer,
    unsyncedCount,
    loadTasks,
    refreshMyTasks,
    loadActiveTimer,
    loadUnsyncedCount,
    startTimer,
    stopTimer,
    runSync,
  } = useStore();
  const [trayAvailable, setTrayAvailable] = useState(true);
  const [syncReport, setSyncReport] = useState<SyncReport | null>(null);
  const [historyRefreshSignal, setHistoryRefreshSignal] = useState(0);

  useEffect(() => {
    loadTasks();
    loadActiveTimer();
    loadUnsyncedCount();
    isTrayAvailable().then(setTrayAvailable);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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

  if (!settings) return null;

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
      <TimerWidget
        activeTimer={activeTimer}
        tasks={allTasks}
        onStart={handleStartTimer}
        onStop={handleStopTimer}
      />
      <div className="panels-row">
        <MyTasksPanel tasks={myTasks} onRefresh={refreshMyTasks} onStartTimer={handleStartTimer} />
        <FavoritesPanel tasks={favoriteTasks} onChanged={loadTasks} onStartTimer={handleStartTimer} />
      </div>
      <HistoryList tasks={allTasks} refreshSignal={historyRefreshSignal} onEntriesChanged={loadUnsyncedCount} />
      {syncReport && <SyncReportModal report={syncReport} onClose={() => setSyncReport(null)} />}
    </div>
  );
}
