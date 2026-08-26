import { useState } from "react";
import type { SettingsDto, SyncReport } from "../api/types";

export type MainViewTab = "tracker" | "statistics";

interface HeaderBarProps {
  settings: SettingsDto;
  unsyncedCount: number;
  trayAvailable: boolean;
  activeView: MainViewTab;
  onChangeView: (view: MainViewTab) => void;
  onSync: () => Promise<SyncReport>;
  onOpenSettings: () => void;
}

export function HeaderBar({
  settings,
  unsyncedCount,
  trayAvailable,
  activeView,
  onChangeView,
  onSync,
  onOpenSettings,
}: HeaderBarProps) {
  const [syncing, setSyncing] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);

  async function handleSync() {
    setSyncing(true);
    setLastError(null);
    try {
      await onSync();
    } catch (err) {
      setLastError(err as string);
    } finally {
      setSyncing(false);
    }
  }

  return (
    <header className="header-bar">
      <div className="header-bar-left">
        <strong>Time Tracker</strong>
        <span className="connected-as">{settings.jiraEmail}</span>
        {!trayAvailable && (
          <span className="tray-fallback-note" title="System tray unavailable on this system; the timer status is shown here instead.">
            (tray unavailable — status shown here)
          </span>
        )}
      </div>
      <div className="view-tabs">
        <button className={activeView === "tracker" ? "active" : ""} onClick={() => onChangeView("tracker")}>
          Tracker
        </button>
        <button className={activeView === "statistics" ? "active" : ""} onClick={() => onChangeView("statistics")}>
          Statistics
        </button>
      </div>
      <div className="header-bar-right">
        {lastError && <span className="error sync-error">{lastError}</span>}
        <button className="link-button" onClick={onOpenSettings}>
          Settings
        </button>
        <button onClick={handleSync} disabled={syncing || unsyncedCount === 0}>
          {syncing ? "Syncing…" : `Sync${unsyncedCount > 0 ? ` (${unsyncedCount})` : ""}`}
        </button>
      </div>
    </header>
  );
}
