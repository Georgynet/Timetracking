import { create } from "zustand";
import * as api from "../api/commands";
import { localDateString } from "../lib/format";
import { applyTheme } from "../theme";
import type {
  ActiveTimer,
  DailySummary,
  Preferences,
  RangeSummary,
  SettingsDto,
  SyncReport,
  Task,
  WorkdaySession,
  WorkdayStatus,
} from "../api/types";

interface AppStore {
  settings: SettingsDto | null;
  preferences: Preferences;
  myTasks: Task[];
  favoriteTasks: Task[];
  activeTimer: ActiveTimer | null;
  unsyncedCount: number;
  lastSyncReport: SyncReport | null;
  activeWorkday: WorkdayStatus | null;
  /** Every `work_days` session recorded on `workdaySessionsDate`, including
   *  already-ended ones — lets a forgotten-to-stop workday be found and corrected once
   *  it's finally ended, for today or any earlier day the widget navigates to. */
  workdaySessions: WorkdaySession[];
  /** The local calendar date (`YYYY-MM-DD`) `workdaySessions` was last loaded for;
   *  defaults to today. */
  workdaySessionsDate: string;
  dailySummary: DailySummary | null;
  weekSummary: RangeSummary | null;
  monthSummary: RangeSummary | null;
  loading: boolean;

  loadSettings: () => Promise<void>;
  loadPreferences: () => Promise<void>;
  savePreferences: (next: Preferences) => Promise<void>;
  loadTasks: () => Promise<void>;
  refreshMyTasks: () => Promise<void>;
  loadActiveTimer: () => Promise<void>;
  loadUnsyncedCount: () => Promise<void>;
  startTimer: (taskId: number, comment?: string) => Promise<void>;
  stopTimer: () => Promise<void>;
  runSync: () => Promise<SyncReport>;
  loadActiveWorkday: () => Promise<void>;
  /** Reloads `workdaySessions`; `date` defaults to whatever `workdaySessionsDate`
   *  currently holds (today, unless the widget has navigated elsewhere). */
  loadWorkdaySessions: (date?: string) => Promise<void>;
  loadDailySummary: () => Promise<void>;
  loadWeekSummary: () => Promise<void>;
  loadMonthSummary: () => Promise<void>;
  loadPeriodSummaries: () => Promise<void>;
  startWorkday: () => Promise<void>;
  endWorkday: () => Promise<void>;
  startBreak: () => Promise<void>;
  endBreak: () => Promise<void>;
}

export const useStore = create<AppStore>((set, get) => ({
  settings: null,
  // Mirrors the backend's defaults so the panels render at a sane size on the very
  // first paint, before `loadPreferences` has come back.
  preferences: {
    myTasksRows: 5,
    favoritesRows: 4,
    currentSprintDefault: true,
    ticketOrder: "recent",
    theme: "system",
    workdayEditingEnabled: false,
  },
  myTasks: [],
  favoriteTasks: [],
  activeTimer: null,
  unsyncedCount: 0,
  lastSyncReport: null,
  activeWorkday: null,
  workdaySessions: [],
  workdaySessionsDate: localDateString(),
  dailySummary: null,
  weekSummary: null,
  monthSummary: null,
  loading: true,

  loadSettings: async () => {
    const settings = await api.getSettings();
    set({ settings, loading: false });
  },

  loadPreferences: async () => {
    const preferences = await api.getPreferences();
    set({ preferences });
    // The DB is the real store; `initTheme` only applied the localStorage hint.
    applyTheme(preferences.theme);
  },

  savePreferences: async (next) => {
    const preferences = await api.savePreferences(next);
    set({ preferences });
    applyTheme(preferences.theme);
  },

  loadTasks: async () => {
    const [myTasks, favoriteTasks] = await Promise.all([
      api.listMyTasks(),
      api.listFavoriteTasks(),
    ]);
    set({ myTasks, favoriteTasks });
  },

  refreshMyTasks: async () => {
    const myTasks = await api.refreshMyTasks();
    set({ myTasks });
  },

  loadActiveTimer: async () => {
    const activeTimer = await api.getActiveTimer();
    set({ activeTimer });
  },

  loadUnsyncedCount: async () => {
    const unsyncedCount = await api.listUnsyncedCount();
    set({ unsyncedCount });
  },

  startTimer: async (taskId, comment) => {
    // Refresh from the backend even on failure — if our cached `activeTimer` was
    // already stale (e.g. the running entry was ended by some other path), this is
    // what lets the UI self-correct instead of getting stuck showing a phantom state.
    try {
      await api.startTimer(taskId, comment);
    } finally {
      await get().loadActiveTimer();
    }
  },

  stopTimer: async () => {
    try {
      await api.stopTimer();
    } finally {
      await Promise.all([get().loadActiveTimer(), get().loadUnsyncedCount()]);
    }
  },

  runSync: async () => {
    const report = await api.syncAll();
    set({ lastSyncReport: report });
    await get().loadUnsyncedCount();
    return report;
  },

  loadActiveWorkday: async () => {
    const activeWorkday = await api.getActiveWorkday();
    set({ activeWorkday });
  },

  loadWorkdaySessions: async (date) => {
    const targetDate = date ?? get().workdaySessionsDate;
    const workdaySessions = await api.getWorkdaySessions(targetDate);
    set({ workdaySessions, workdaySessionsDate: targetDate });
  },

  loadDailySummary: async () => {
    const dailySummary = await api.getDailySummary();
    set({ dailySummary });
  },

  loadWeekSummary: async () => {
    const weekSummary = await api.getWeekSummary();
    set({ weekSummary });
  },

  loadMonthSummary: async () => {
    const monthSummary = await api.getMonthSummary();
    set({ monthSummary });
  },

  loadPeriodSummaries: async () => {
    await Promise.all([get().loadDailySummary(), get().loadWeekSummary(), get().loadMonthSummary()]);
  },

  startWorkday: async () => {
    try {
      await api.startWorkday();
    } finally {
      // Starting/ending a workday always happens today, regardless of which date the
      // sessions list is currently navigated to — so reload today's specifically
      // rather than whatever `workdaySessionsDate` happens to be.
      await Promise.all([
        get().loadActiveWorkday(),
        get().loadPeriodSummaries(),
        get().loadWorkdaySessions(localDateString()),
      ]);
    }
  },

  endWorkday: async () => {
    try {
      await api.endWorkday();
    } finally {
      await Promise.all([
        get().loadActiveWorkday(),
        get().loadPeriodSummaries(),
        get().loadWorkdaySessions(localDateString()),
      ]);
    }
  },

  startBreak: async () => {
    try {
      await api.startBreak();
    } finally {
      await get().loadActiveWorkday();
    }
  },

  endBreak: async () => {
    try {
      await api.endBreak();
    } finally {
      await Promise.all([get().loadActiveWorkday(), get().loadPeriodSummaries()]);
    }
  },
}));
