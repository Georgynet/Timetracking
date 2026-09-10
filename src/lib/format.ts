export function toDateInput(iso: string): string {
  return iso.slice(0, 10);
}

export function toTimeInput(iso: string): string {
  const d = new Date(iso);
  return `${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}`;
}

export function combine(date: string, time: string): string {
  return new Date(`${date}T${time}`).toISOString();
}

export function formatDuration(seconds: number | null): string {
  if (seconds === null) return "—";
  const h = Math.floor(seconds / 3600);
  const m = Math.round((seconds % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

/** Renders a whole number of minutes as a compact Jira-style duration (e.g. `95` -> `"1h 35m"`),
 * for prefilling the duration input — the counterpart to `parseDurationInput`. */
export function formatDurationInput(totalMinutes: number): string {
  const h = Math.floor(totalMinutes / 60);
  const m = totalMinutes % 60;
  if (h > 0 && m > 0) return `${h}h ${m}m`;
  if (h > 0) return `${h}h`;
  return `${m}m`;
}

/** Parses a Jira-style duration (`"1h 35m"`, `"1h35m"`, `"1.5h"`, `"90m"`) into whole minutes.
 * A bare number (`"90"`) is accepted as minutes for backward compatibility. Returns `null` for
 * anything that isn't a positive duration in this shape. */
export function parseDurationInput(input: string): number | null {
  const trimmed = input.trim().toLowerCase();
  if (!trimmed) return null;

  if (/^\d+(\.\d+)?$/.test(trimmed)) {
    const minutes = Math.round(parseFloat(trimmed));
    return minutes > 0 ? minutes : null;
  }

  const match = trimmed.match(/^(?:(\d+(?:\.\d+)?)\s*h)?\s*(?:(\d+(?:\.\d+)?)\s*m)?$/);
  if (!match || (!match[1] && !match[2])) return null;

  const hours = match[1] ? parseFloat(match[1]) : 0;
  const minutes = match[2] ? parseFloat(match[2]) : 0;
  const totalMinutes = Math.round(hours * 60 + minutes);
  return totalMinutes > 0 ? totalMinutes : null;
}

/** Clock-style `H:MM:SS` (or `M:SS` under an hour) for a live-ticking elapsed display. */
export function formatElapsed(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}
