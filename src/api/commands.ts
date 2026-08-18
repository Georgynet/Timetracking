import { invoke } from "@tauri-apps/api/core";
import type {
  ActiveTimer,
  DailySummary,
  Granularity,
  IntervalBucket,
  JiraIssue,
  JiraMyself,
  RangeSummary,
  SettingsDto,
  SyncReport,
  Task,
  TicketTotal,
  TimeEntry,
  WorkBreak,
  WorkDay,
  WorkdayStatus,
} from "./types";

export const getSettings = () => invoke<SettingsDto>("get_settings");

export const saveJiraSettings = (baseUrl: string, email: string, apiToken: string) =>
  invoke<JiraMyself>("save_jira_settings", { baseUrl, email, apiToken });

export const testJiraConnection = () => invoke<JiraMyself>("test_jira_connection");

export const clearJiraSettings = () => invoke<void>("clear_jira_settings");

export const refreshMyTasks = () => invoke<Task[]>("refresh_my_tasks");

export const listMyTasks = () => invoke<Task[]>("list_my_tasks");

export const listFavoriteTasks = () => invoke<Task[]>("list_favorite_tasks");

export const searchJiraIssues = (query: string) =>
  invoke<JiraIssue[]>("search_jira_issues", { query });

export const addFavoriteByKey = (jiraKey: string) =>
  invoke<Task>("add_favorite_by_key", { jiraKey });

export const removeFavorite = (taskId: number) =>
  invoke<void>("remove_favorite", { taskId });

export const getActiveTimer = () => invoke<ActiveTimer | null>("get_active_timer");

export const startTimer = (taskId: number, comment?: string) =>
  invoke<TimeEntry>("start_timer", { taskId, comment: comment ?? null });

export const stopTimer = () => invoke<TimeEntry>("stop_timer");

export const createManualEntry = (params: {
  taskId: number;
  startedAt: string;
  endedAt?: string;
  durationSeconds?: number;
  comment?: string;
}) =>
  invoke<TimeEntry>("create_manual_entry", {
    taskId: params.taskId,
    startedAt: params.startedAt,
    endedAt: params.endedAt ?? null,
    durationSeconds: params.durationSeconds ?? null,
    comment: params.comment ?? null,
  });

export const updateTimeEntry = (params: {
  id: number;
  taskId?: number;
  startedAt?: string;
  endedAt?: string;
  durationSeconds?: number;
  /** Omit to leave the comment unchanged; pass "" to clear it, or new text to set it. */
  comment?: string;
}) =>
  invoke<TimeEntry>("update_time_entry", {
    id: params.id,
    taskId: params.taskId ?? null,
    startedAt: params.startedAt ?? null,
    endedAt: params.endedAt ?? null,
    durationSeconds: params.durationSeconds ?? null,
    comment: params.comment ?? null,
  });

export const listTimeEntries = (params?: { taskId?: number; from?: string; to?: string }) =>
  invoke<TimeEntry[]>("list_time_entries", {
    taskId: params?.taskId ?? null,
    from: params?.from ?? null,
    to: params?.to ?? null,
  });

export const deleteDraftEntry = (id: number) => invoke<void>("delete_draft_entry", { id });

export const syncAll = () => invoke<SyncReport>("sync_all");

export const listUnsyncedCount = () => invoke<number>("list_unsynced_count");

export const isTrayAvailable = () => invoke<boolean>("is_tray_available");

export const getActiveWorkday = () => invoke<WorkdayStatus | null>("get_active_workday");

export const startWorkday = () => invoke<WorkDay>("start_workday");

export const endWorkday = () => invoke<WorkDay>("end_workday");

export const startBreak = () => invoke<WorkBreak>("start_break");

export const endBreak = () => invoke<WorkBreak>("end_break");

export const updateBreak = (params: { id: number; startedAt: string; endedAt: string }) =>
  invoke<WorkBreak>("update_break", {
    id: params.id,
    startedAt: params.startedAt,
    endedAt: params.endedAt,
  });

export const getDailySummary = (date?: string) =>
  invoke<DailySummary>("get_daily_summary", { date: date ?? null });

export const getWeekSummary = () => invoke<RangeSummary>("get_week_summary");

export const getMonthSummary = () => invoke<RangeSummary>("get_month_summary");

export const getTicketStats = (params: { from: string; to: string }) =>
  invoke<TicketTotal[]>("get_ticket_stats", { from: params.from, to: params.to });

export const getIntervalStats = (params: { from: string; to: string; granularity: Granularity }) =>
  invoke<IntervalBucket[]>("get_interval_stats", {
    from: params.from,
    to: params.to,
    granularity: params.granularity,
  });
