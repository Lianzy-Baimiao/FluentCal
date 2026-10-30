import type { ISODate, Task } from '@/types';
import { addDays } from './date';
import { occurrences, type Occurrence } from './recur';

export interface AgendaDay {
  date: ISODate;
  items: Occurrence[];
}

/**
 * The web view's list (WEB-03): tasks from `start` to `end`, by day. A task that began before
 * `start` and is still running is listed on `start`. Days without tasks are left out, except
 * `today`.
 */
export function agenda(tasks: Task[], start: ISODate, end: ISODate, today: ISODate, showDone = true): AgendaDay[] {
  const byDay = new Map<ISODate, Occurrence[]>();
  for (const t of tasks) {
    for (const o of occurrences(t, start, end, 200)) {
      if (!showDone && o.done) continue;
      const day = o.start < start ? start : o.start;
      if (day > end) continue;
      (byDay.get(day) ?? byDay.set(day, []).get(day)!).push(o);
    }
  }
  if (today >= start && today <= end && !byDay.has(today)) byDay.set(today, []);
  // All-day first, then by time; within that the calendar's own order.
  const key = (o: Occurrence) => `${o.task.time ?? ''}`.padStart(5, ' ');
  return [...byDay.entries()]
    .sort(([a], [b]) => (a < b ? -1 : 1))
    .map(([date, items]) => ({
      date,
      items: items.sort((a, b) => key(a).localeCompare(key(b)) || +a.done - +b.done || a.task.order - b.task.order || a.task.title.localeCompare(b.task.title)),
    }));
}

/** "今天", "明天", or e.g. "10月31日". */
export function dayHeading(date: ISODate, today: ISODate): string {
  if (date === today) return '今天';
  if (date === addDays(today, 1)) return '明天';
  if (date === addDays(today, -1)) return '昨天';
  return `${+date.slice(5, 7)}月${+date.slice(8, 10)}日`;
}
