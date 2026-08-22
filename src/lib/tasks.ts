import type { Task, TicketOrder } from "../api/types";

/**
 * Orders tickets for the pickers. "recent" puts what you last tracked first — the
 * next thing you track is usually something you tracked lately — with never-tracked
 * tickets after them, alphabetically, so the tail stays predictable rather than
 * arbitrary. "key" is the plain alphabetical order the backend already returns.
 *
 * Returns a new array; callers pass store-owned arrays that must not be mutated.
 */
export function orderTasks(tasks: Task[], order: TicketOrder): Task[] {
  if (order === "key") return [...tasks].sort((a, b) => a.jiraKey.localeCompare(b.jiraKey));

  return [...tasks].sort((a, b) => {
    if (a.lastTrackedAt && b.lastTrackedAt) return b.lastTrackedAt.localeCompare(a.lastTrackedAt);
    if (a.lastTrackedAt) return -1;
    if (b.lastTrackedAt) return 1;
    return a.jiraKey.localeCompare(b.jiraKey);
  });
}
