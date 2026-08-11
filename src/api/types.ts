export interface Task {
  id: number;
  jiraKey: string;
  summary: string;
  isFavorite: boolean;
  isAssignedToMe: boolean;
  lastSyncedAt: string | null;
}

export interface TimeEntry {
  id: number;
  taskId: number;
  taskKey: string;
  taskSummary: string;
  startedAt: string;
  endedAt: string | null;
  durationSeconds: number | null;
  comment: string | null;
  isSynced: boolean;
  jiraWorklogId: string | null;
  createdManually: boolean;
  editedAt: string | null;
}

export interface ActiveTimer extends TimeEntry {
  isStale: boolean;
}

export interface SettingsDto {
  jiraBaseUrl: string | null;
  jiraEmail: string | null;
  hasToken: boolean;
}

export interface JiraMyself {
  accountId: string;
  displayName: string;
  emailAddress: string;
}

export interface JiraIssue {
  key: string;
  summary: string;
  status: string | null;
  project: string | null;
}

export interface SyncFailure {
  entryId: number;
  taskKey: string;
  message: string;
}

export interface SyncReport {
  total: number;
  succeeded: number[];
  failed: SyncFailure[];
}

/** Tauri commands reject with a plain string (see `error::AppError`'s `Serialize` impl). */
export type CommandError = string;
