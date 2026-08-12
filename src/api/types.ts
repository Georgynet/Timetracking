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

export interface WorkDay {
  id: number;
  workDate: string;
  startedAt: string;
  endedAt: string | null;
}

export interface WorkBreak {
  id: number;
  workDayId: number;
  startedAt: string;
  endedAt: string | null;
}

export interface WorkdayStatus extends WorkDay {
  breaks: WorkBreak[];
  isOnBreak: boolean;
  /** Worked/break seconds already banked today from earlier, already-ended sessions. */
  priorWorkedSecondsToday: number;
  priorBreakSecondsToday: number;
}

export interface DailySummary {
  date: string;
  workedSeconds: number;
  loggedSeconds: number;
  diffSeconds: number;
}

export interface RangeSummary {
  from: string;
  to: string;
  workedSeconds: number;
  loggedSeconds: number;
  diffSeconds: number;
}

/** Tauri commands reject with a plain string (see `error::AppError`'s `Serialize` impl). */
export type CommandError = string;
