import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { SyncReport } from "./types";

// Event names mirror the constants in `src-tauri/src/tray.rs`.

/** Fired after a tray menu action changed timer/workday state behind the UI's back. */
export const onTrayStateChanged = (handler: () => void): Promise<UnlistenFn> =>
  listen("tray-state-changed", () => handler());

/** Fired when a sync started from the tray finishes, with the same report `syncAll` returns. */
export const onTraySyncFinished = (handler: (report: SyncReport) => void): Promise<UnlistenFn> =>
  listen<SyncReport>("tray-sync-finished", (event) => handler(event.payload));

/** Fired by the tray's "Ticket key…" item: the window is shown and should focus the
 * start-by-key field (a native menu can't take text input itself). */
export const onTrayStartByKey = (handler: () => void): Promise<UnlistenFn> =>
  listen("tray-start-by-key", () => handler());
