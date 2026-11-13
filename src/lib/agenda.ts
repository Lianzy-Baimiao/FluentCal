import { t as tr } from '@/i18n';
import type { ISODate, Task } from '@/types';
import { addDays, parts } from './date';
import { occurrences, type Occurrence } from './recur';

export interface AgendaDay {
  date: ISODate;
  items: Occurrence[];
}

/**
 * The list (WEB-03), 近期: tasks from `start` to `end`, by day. A task that began before
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
  return sorted(byDay);
}

/**
 * 所有 (GitHub #4): every task once, on its start date, so what was recorded before can be
 * looked through. A repeating task stands for its series: never done here, its rule says when
 * it comes. Today is always there (the list opens on it). Same as the desktop's agenda.rs.
 */
export function allTasks(tasks: Task[], today: ISODate, showDone = true): AgendaDay[] {
  const byDay = new Map<ISODate, Occurrence[]>();
  for (const t of tasks) {
    const done = !t.repeat && t.done;
    if ((!showDone && done) || !t.start) continue;
    const o: Occurrence = { task: t, date: t.start, start: t.start, end: t.end && t.end >= t.start ? t.end : t.start, done };
    (byDay.get(t.start) ?? byDay.set(t.start, []).get(t.start)!).push(o);
  }
  if (!byDay.has(today)) byDay.set(today, []);
  return sorted(byDay);
}

/** All-day first, then by time; within that open before done, then the calendar's own order. */
function sorted(byDay: Map<ISODate, Occurrence[]>): AgendaDay[] {
  const key = (o: Occurrence) => `${o.task.time ?? ''}`.padStart(5, ' ');
  return [...byDay.entries()]
    .sort(([a], [b]) => (a < b ? -1 : 1))
    .map(([date, items]) => ({
      date,
      items: items.sort((a, b) => key(a).localeCompare(key(b)) || +a.done - +b.done || a.task.order - b.task.order || a.task.title.localeCompare(b.task.title)),
    }));
}

/** "今天", "明天", "昨天", or e.g. "10月31日"; with the year when it isn't this year's. */
export function dayHeading(date: ISODate, today: ISODate): string {
  if (date === today) return tr("今天");
  if (date === addDays(today, 1)) return tr("明天");
  if (date === addDays(today, -1)) return tr("昨天");
  const [y, m, d] = parts(date);
  return y === parts(today)[0] ? tr("{0}月{1}日", m, d) : tr("{0}年{1}月{2}日", y, m, d);
}
