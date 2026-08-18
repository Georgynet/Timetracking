import { openUrl } from "@tauri-apps/plugin-opener";
import { useState } from "react";
import { addFavoriteByKey, removeFavorite, searchJiraIssues } from "../api/commands";
import type { JiraIssue, Task } from "../api/types";
import { jiraIssueUrl } from "../lib/jira";

interface FavoritesPanelProps {
  tasks: Task[];
  jiraBaseUrl: string;
  onChanged: () => Promise<void>;
  onStartTimer: (taskId: number) => Promise<void>;
}

export function FavoritesPanel({ tasks, jiraBaseUrl, onChanged, onStartTimer }: FavoritesPanelProps) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<JiraIssue[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSearch() {
    if (!query.trim()) return;
    setSearching(true);
    setError(null);
    try {
      setResults(await searchJiraIssues(query.trim()));
    } catch (err) {
      setError(err as string);
      setResults([]);
    } finally {
      setSearching(false);
    }
  }

  function handleClearSearch() {
    setQuery("");
    setResults([]);
    setError(null);
  }

  async function handleAdd(key: string) {
    setError(null);
    try {
      await addFavoriteByKey(key);
      setResults([]);
      setQuery("");
      await onChanged();
    } catch (err) {
      setError(err as string);
    }
  }

  async function handleRemove(taskId: number) {
    await removeFavorite(taskId);
    await onChanged();
  }

  return (
    <section className="panel">
      <div className="panel-header">
        <h2>Favorites</h2>
      </div>
      <div className="favorite-search">
        <input
          type="text"
          placeholder="Search by key (TEAM-1) or free text in Jira…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleSearch()}
        />
        <button onClick={handleSearch} disabled={searching || !query.trim()}>
          {searching ? "Searching…" : "Search"}
        </button>
        {(results.length > 0 || query) && (
          <button onClick={handleClearSearch} disabled={searching}>
            Clear
          </button>
        )}
      </div>
      {error && <p className="error">{error}</p>}
      {results.length === 0 && tasks.length === 0 ? (
        <p className="empty-hint">No favorites yet — search for a ticket above.</p>
      ) : (
        // Search results and saved favorites share one list — one scrollable area
        // capped at 4 rows total, rather than two separate capped lists stacked on
        // top of each other.
        <ul className="task-list task-list-capped favorites-list">
          {results.map((issue) => (
            <li key={`result-${issue.key}`}>
              <span className="task-key">{issue.key}</span>
              <span className="task-summary">{issue.summary}</span>
              <button onClick={() => handleAdd(issue.key)}>Add favorite</button>
            </li>
          ))}
          {tasks.map((t) => (
            <li key={`favorite-${t.id}`}>
              <a
                className="task-key jira-link"
                href={jiraIssueUrl(jiraBaseUrl, t.jiraKey)}
                onClick={(e) => {
                  e.preventDefault();
                  openUrl(jiraIssueUrl(jiraBaseUrl, t.jiraKey));
                }}
              >
                {t.jiraKey}
              </a>
              <span className="task-summary">{t.summary}</span>
              <button onClick={() => onStartTimer(t.id)}>Start</button>
              <button className="link-button" onClick={() => handleRemove(t.id)}>
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
