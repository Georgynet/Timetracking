import { useEffect } from "react";
import "./App.css";
import { useStore } from "./state/store";
import { MainView } from "./views/MainView";
import { SetupView } from "./views/SetupView";

function App() {
  const { settings, loading, loadSettings, loadPreferences } = useStore();

  useEffect(() => {
    loadSettings();
    // Loaded here rather than in MainView so the theme applies on the setup screen
    // too — it's the first thing a new user sees.
    loadPreferences();
  }, [loadSettings, loadPreferences]);

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
