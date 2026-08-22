import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ThemePreference } from "./api/types";

const STORAGE_KEY = "ui.theme";
const DARK_QUERY = "(prefers-color-scheme: dark)";

/**
 * Theme application. The preference is three-way (system/light/dark) but the DOM only
 * ever carries the *resolved* scheme as `data-theme="light" | "dark"`, so the
 * stylesheet needs a single dark block rather than one per path (see ADR-0027).
 *
 * "system" is resolved here rather than by a `prefers-color-scheme` media query so
 * that both paths share those declarations, and re-resolved live by the listener
 * below — flipping macOS between light and dark (including its automatic day/night
 * schedule) repaints the app without a restart.
 */

/** `window.matchMedia()` returns a *new* `MediaQueryList` on every call, so a listener
 *  can only be removed through the very object it was added to. Holding one for the
 *  module's lifetime is what makes the detach below actually detach: otherwise every
 *  apply leaks a listener, and those leaked listeners keep rewriting `data-theme` from
 *  the OS scheme after the user has explicitly chosen Light or Dark. */
const darkMedia = window.matchMedia(DARK_QUERY);

let systemListener: (() => void) | null = null;

function isValid(value: string | null): value is ThemePreference {
  return value === "system" || value === "light" || value === "dark";
}

function resolve(preference: ThemePreference): "light" | "dark" {
  if (preference !== "system") return preference;
  return darkMedia.matches ? "dark" : "light";
}

/**
 * Keeps the native title bar in step with the page. Best-effort by design: outside
 * Tauri (`npm run dev` in a plain browser) `getCurrentWindow()` throws synchronously,
 * and the command itself rejects asynchronously if the window ACL ever stops granting
 * `core:window:allow-set-theme` — so both paths are swallowed. The page is already
 * correct either way; only the chrome would be out of step.
 */
function setWindowTheme(preference: ThemePreference) {
  try {
    getCurrentWindow()
      .setTheme(preference === "system" ? null : preference)
      .catch(() => {});
  } catch {
    // No Tauri window to theme.
  }
}

export function applyTheme(preference: ThemePreference) {
  document.documentElement.dataset.theme = resolve(preference);

  try {
    localStorage.setItem(STORAGE_KEY, preference);
  } catch {
    // Only costs us the flash-free launch below; the DB remains the real store.
  }

  if (systemListener) {
    darkMedia.removeEventListener("change", systemListener);
    systemListener = null;
  }
  if (preference === "system") {
    systemListener = () => {
      document.documentElement.dataset.theme = resolve("system");
    };
    darkMedia.addEventListener("change", systemListener);
  }

  setWindowTheme(preference);
}

/**
 * Applies the last known preference before the first paint. The real value lives in
 * the app DB and arrives asynchronously, so it's mirrored to localStorage on every
 * apply purely so an explicit Dark user on a light system doesn't get a white frame
 * at launch — `index.html` reads the same key even earlier, before this module runs.
 * The DB value re-applies moments later, normally as a no-op.
 */
export function initTheme() {
  let stored: string | null = null;
  try {
    stored = localStorage.getItem(STORAGE_KEY);
  } catch {
    // Private mode or blocked storage — fall through to the system scheme.
  }
  applyTheme(isValid(stored) ? stored : "system");
}
