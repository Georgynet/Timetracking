export function jiraIssueUrl(baseUrl: string, key: string): string {
  return `${baseUrl.replace(/\/+$/, "")}/browse/${key}`;
}

/** Pulls an issue key out of what was typed or pasted — a bare key (`proj-123`) or a
 * Jira link (`…/browse/PROJ-123`, `…?selectedIssue=PROJ-123`). The *last* key-shaped
 * match wins, so a hostname like `jira-2.example.com` doesn't shadow the real key. */
export function extractIssueKey(input: string): string | null {
  const matches = input.match(/[A-Za-z][A-Za-z0-9_]*-\d+/g);
  return matches ? matches[matches.length - 1].toUpperCase() : null;
}
