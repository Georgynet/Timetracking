import { create } from "zustand";
import * as api from "../api/commands";
import type {
  ActiveTimer,
  DailySummary,
  SettingsDto,
  SyncReport,
  Task,
  WorkdayStatus,
} from "../api/types";

interface AppStore {
  settings: SettingsDto | null;
  myTasks: Task[];
  favoriteTasks: Task[];
  activeTimer: ActiveTimer | null;
  unsyncedCount: number;
  lastSyncReport: SyncReport | null;
  activeWorkday: WorkdayStatus | null;
  dailySummary: DailySummary | null;
  loading: boolean;

  loadSettings: () => Promise<void>;
  loadTasks: () => Promise<void>;
  refreshMyTasks: () => Promise<void>;
  loadActiveTimer: () => Promise<void>;
  loadUnsyncedCount: () => Promise<void>;
  startTimer: (taskId: number, comment?: string) => Promise<void>;
  stopTimer: () => Promise<void>;
  runSync: () => Promise<SyncReport>;
  loadActiveWorkday: () => Promise<void>;
  loadDailySummary: () => Promise<void>;
  startWorkday: () => Promise<void>;
  endWorkday: () => Promise<void>;
  startBreak: () => Promise<void>;
  endBreak: () => Promise<void>;
}

export const useStore = create<AppStore>((set, get) => ({
  settings: null,
  myTasks: [],
  favoriteTasks: [],
  activeTimer: null,
  unsyncedCount: 0,
  lastSyncReport: null,
  activeWorkday: null,
  dailySummary: null,
  loading: true,

  loadSettings: async () => {
    const settings = await api.getSettings();
    set({ settings, loading: false });
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

  loadDailySummary: async () => {
    const dailySummary = await api.getDailySummary();
    set({ dailySummary });
  },

  startWorkday: async () => {
    try {
      await api.startWorkday();
    } finally {
      await Promise.all([get().loadActiveWorkday(), get().loadDailySummary()]);
    }
  },

  endWorkday: async () => {
    try {
      await api.endWorkday();
    } finally {
      await Promise.all([get().loadActiveWorkday(), get().loadDailySummary()]);
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
      await Promise.all([get().loadActiveWorkday(), get().loadDailySummary()]);
    }
  },
}));
