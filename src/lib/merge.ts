import type { Task } from '@/types';

const same = (a: unknown, b: unknown) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);

/**
 * Three-way merge of an edited task: fields the user changed (draft vs. base) win, every
 * other field keeps the current stored value, so edits made elsewhere meanwhile (ticked
 * done, dragged in the Gantt) survive.
 */
export function mergeEdit(base: Task, draft: Task, current: Task): Task {
  const out: Record<string, unknown> = { ...current };
  const keys = new Set([...Object.keys(base), ...Object.keys(draft)]);
  for (const k of keys) {
    const d = (draft as unknown as Record<string, unknown>)[k];
    if (!same(d, (base as unknown as Record<string, unknown>)[k])) out[k] = d;
  }
  const t = out as unknown as Task;
  if (t.milestone || t.end < t.start) t.end = t.start;
  return t;
}
