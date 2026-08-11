import { FormEvent, useState } from "react";
import { saveJiraSettings } from "../api/commands";
import type { CommandError } from "../api/types";

export function SetupView({ onSaved }: { onSaved: () => void }) {
  const [baseUrl, setBaseUrl] = useState("");
  const [email, setEmail] = useState("");
  const [apiToken, setApiToken] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      await saveJiraSettings(baseUrl, email, apiToken);
      onSaved();
    } catch (err) {
      setError(err as CommandError);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <main className="setup-view">
      <h1>Connect to Jira</h1>
      <p className="hint">
        Enter your Jira Cloud instance URL, your account email, and an API token. You
        can create a token at{" "}
        <span className="mono">id.atlassian.com/manage-profile/security/api-tokens</span>.
      </p>
      <form onSubmit={handleSubmit}>
        <label>
          Instance URL
          <input
            type="text"
            placeholder="https://your-company.atlassian.net"
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            required
          />
        </label>
        <label>
          Email
          <input
            type="email"
            placeholder="you@company.com"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
          />
        </label>
        <label>
          API token
          <input
            type="password"
            value={apiToken}
            onChange={(e) => setApiToken(e.target.value)}
            required
          />
        </label>
        {error && <p className="error">{error}</p>}
        <button type="submit" disabled={submitting}>
          {submitting ? "Verifying…" : "Connect"}
        </button>
      </form>
    </main>
  );
}
