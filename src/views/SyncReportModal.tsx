import type { SyncReport } from "../api/types";

export function SyncReportModal({ report, onClose }: { report: SyncReport; onClose: () => void }) {
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>Sync results</h2>
        <p>
          {report.succeeded.length} of {report.total} entries synced successfully.
        </p>
        {report.failed.length > 0 && (
          <>
            <p className="error">{report.failed.length} entries failed and remain pending:</p>
            <ul className="sync-failures">
              {report.failed.map((f) => (
                <li key={f.entryId}>
                  <strong>{f.taskKey}</strong>: {f.message}
                </li>
              ))}
            </ul>
            <p className="hint">
              These entries were left untouched and will be retried the next time you click Sync.
            </p>
          </>
        )}
        <div className="modal-actions">
          <button onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}
