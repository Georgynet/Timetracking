import { openUrl } from "@tauri-apps/plugin-opener";
import type { TicketTotal } from "../api/types";
import { formatDuration } from "../lib/format";
import { jiraIssueUrl } from "../lib/jira";

interface TicketStatsTableProps {
  totals: TicketTotal[];
  jiraBaseUrl: string;
}

export function TicketStatsTable({ totals, jiraBaseUrl }: TicketStatsTableProps) {
  if (totals.length === 0) {
    return <p className="empty-hint">No time logged in this range.</p>;
  }

  const totalSeconds = totals.reduce((sum, total) => sum + total.totalSeconds, 0);

  return (
    <table className="stats-table">
      <thead>
        <tr>
          <th>Ticket</th>
          <th>Summary</th>
          <th>Total time</th>
        </tr>
      </thead>
      <tbody>
        {totals.map((total) => (
          <tr key={total.taskId}>
            <td>
              <a
                className="jira-link"
                href={jiraIssueUrl(jiraBaseUrl, total.taskKey)}
                onClick={(e) => {
                  e.preventDefault();
                  openUrl(jiraIssueUrl(jiraBaseUrl, total.taskKey));
                }}
              >
                {total.taskKey}
              </a>
            </td>
            <td className="task-summary">{total.taskSummary}</td>
            <td>{formatDuration(total.totalSeconds)}</td>
          </tr>
        ))}
      </tbody>
      {/* The same summary the History table carries, for the range this view is showing.
          It counts tickets rather than entries because tickets are what the rows are, and
          the total sums exactly the column above it. */}
      <tfoot>
        <tr>
          <th scope="row">Tickets</th>
          <td>{totals.length}</td>
          <td>{formatDuration(totalSeconds)}</td>
        </tr>
      </tfoot>
    </table>
  );
}
