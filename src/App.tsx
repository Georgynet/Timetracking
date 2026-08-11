import { useEffect } from "react";
import "./App.css";
import { useStore } from "./state/store";
import { MainView } from "./views/MainView";
import { SetupView } from "./views/SetupView";

function App() {
  const { settings, loading, loadSettings } = useStore();

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  if (loading) {
    return <main className="loading-screen">Loading…</main>;
  }

  const isConfigured = Boolean(settings?.jiraBaseUrl && settings?.hasToken);

  if (!isConfigured) {
    return <SetupView onSaved={loadSettings} />;
  }

  return <MainView onReconfigure={loadSettings} />;
}

export default App;
