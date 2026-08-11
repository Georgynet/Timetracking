export function jiraIssueUrl(baseUrl: string, key: string): string {
  return `${baseUrl.replace(/\/+$/, "")}/browse/${key}`;
}
