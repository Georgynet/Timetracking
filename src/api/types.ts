export interface Task {
  id: number;
  jiraKey: string;
  summary: string;
  isFavorite: boolean;
  isAssignedToMe: boolean;
  isInCurrentSprint: boolean;
  lastSyncedAt: string | null;
  /** When a timer was last started on this ticket; null if never tracked. */
  lastTrackedAt: string | null;
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

/** UI preferences, stored in the app DB (see `commands::preferences`). */
export interface Preferences {
  myTasksRows: number;
  favoritesRows: number;
  /** Whether My Tasks starts filtered to the current sprint on launch. */
  currentSprintDefault: boolean;
  /** Ordering for the ticket pickers. */
  ticketOrder: TicketOrder;
  /** Light, dark, or whatever the OS is set to. */
  theme: ThemePreference;
}

export type ThemePreference = "system" | "light" | "dark";

export type TicketOrder = "recent" | "key";

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

export type Granularity = "day" | "week" | "month";

export interface TicketTotal {
  taskId: number;
  taskKey: string;
  taskSummary: string;
  totalSeconds: number;
}

export interface TicketSeconds {
  taskId: number;
  taskKey: string;
  seconds: number;
}

export interface IntervalBucket {
  periodStart: string;
  periodEnd: string;
  tickets: TicketSeconds[];
  breakSeconds: number;
}

/** Tauri commands reject with a plain string (see `error::AppError`'s `Serialize` impl). */
export type CommandError = string;
