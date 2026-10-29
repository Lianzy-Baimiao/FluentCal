import type { Group, ISODate, Task } from '@/types';
import { addMonths, diffDays } from './date';
import { occurrences } from './recur';

export interface SearchOptions {
  q: string;
  range: 'all' | '1m' | '3m' | '1y';
  status: 'all' | 'done' | 'undone';
  /** Color key/hex; '' = any. */
  color: string;
}

export interface SearchHit {
  task: Task;
  /** Date to jump to: the task start, or for a repeating task its occurrence nearest today. */
  date: ISODate;
}

/** Occurrence of `t` closest to `now` within [lo, hi]. */
function nearest(t: Task, now: ISODate, lo: ISODate, hi: ISODate): ISODate | undefined {
  if (!t.repeat) return t.end < lo || t.start > hi ? undefined : t.start;
  let best: ISODate | undefined;
  for (const o of occurrences(t, lo, hi, 1000)) {
    if (best === undefined || Math.abs(diffDays(now, o.start)) < Math.abs(diffDays(now, best))) best = o.start;
    if (o.start >= now) break; // occurrences are ordered; later ones are farther away
  }
  return best;
}

/** Tasks matching the query (title, notes, group name), nearest to today first. */
export function searchTasks(tasks: Task[], groups: Record<string, Group>, o: SearchOptions, now: ISODate): SearchHit[] {
  const q = o.q.trim().toLowerCase();
  const months = { all: 0, '1m': 1, '3m': 3, '1y': 12 }[o.range];
  // "All time" still needs a window for repeating tasks: ten years around today.
  const lo = addMonths(now, -(months || 120));
  const hi = addMonths(now, months || 120);
  const hits: SearchHit[] = [];
  for (const t of tasks) {
    if (q) {
      const hay = `${t.title}\n${t.notes ?? ''}\n${(t.groupId && groups[t.groupId]?.name) || ''}`.toLowerCase();
      if (!q.split(/\s+/).every((w) => hay.includes(w))) continue;
    }
    if (o.status === 'done' && !t.done) continue;
    if (o.status === 'undone' && t.done) continue;
    if (o.color && (t.color ?? '') !== o.color) continue;
    const date = months || t.repeat ? nearest(t, now, lo, hi) : t.start;
    if (date) hits.push({ task: t, date });
  }
  return hits.sort((a, b) => Math.abs(diffDays(now, a.date)) - Math.abs(diffDays(now, b.date)) || a.task.order - b.task.order);
}
