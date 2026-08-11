import { confirm } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { clearJiraSettings } from "../api/commands";
import type { SettingsDto, SyncReport } from "../api/types";

interface HeaderBarProps {
  settings: SettingsDto;
  unsyncedCount: number;
  trayAvailable: boolean;
  onSync: () => Promise<SyncReport>;
  onReconfigure: () => void;
}

export function HeaderBar({ settings, unsyncedCount, trayAvailable, onSync, onReconfigure }: HeaderBarProps) {
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

  async function handleReconfigure() {
    const confirmed = await confirm("Disconnect from Jira and re-enter your credentials?", {
      title: "Reconfigure Jira",
      kind: "warning",
    });
    if (!confirmed) return;
    await clearJiraSettings();
    onReconfigure();
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
      <div className="header-bar-right">
        {lastError && <span className="error sync-error">{lastError}</span>}
        <button onClick={handleSync} disabled={syncing || unsyncedCount === 0}>
          {syncing ? "Syncing…" : `Sync${unsyncedCount > 0 ? ` (${unsyncedCount})` : ""}`}
        </button>
        <button className="link-button" onClick={handleReconfigure}>
          Reconfigure
        </button>
      </div>
    </header>
  );
}
