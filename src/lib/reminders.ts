import type { ISODate, Task } from '@/types';
import { parts, relativeDay, toISO } from './date';
import { occurrences } from './recur';

export interface ReminderItem {
  key: string;
  atMs: number;
  title: string;
  body: string;
  taskId: string;
  date: ISODate;
}

export const REMINDER_OFFSETS: { value: number; label: string }[] = [
  { value: 0, label: '准时' },
  { value: 5, label: '提前 5 分钟' },
  { value: 10, label: '提前 10 分钟' },
  { value: 15, label: '提前 15 分钟' },
  { value: 30, label: '提前 30 分钟' },
  { value: 60, label: '提前 1 小时' },
  { value: 120, label: '提前 2 小时' },
  { value: 1440, label: '提前 1 天' },
  { value: 2880, label: '提前 2 天' },
  { value: 4320, label: '提前 3 天' },
  { value: 7200, label: '提前 5 天' },
  { value: 10080, label: '提前 1 周' },
];

export function offsetLabel(min: number): string {
  const hit = REMINDER_OFFSETS.find((o) => o.value === min);
  if (hit) return hit.label;
  if (min % 1440 === 0) return `提前 ${min / 1440} 天`;
  if (min % 60 === 0) return `提前 ${min / 60} 小时`;
  return `提前 ${min} 分钟`;
}

/**
 * Reminders firing in (now - grace, now + horizon]. All-day tasks fire at `allDayTime` on the
 * occurrence day (minus the offset). Done occurrences are skipped.
 */
export function computeReminders(
  tasks: Task[],
  nowMs: number,
  allDayTime: string,
  horizonMs = 48 * 3600_000,
  graceMs = 10 * 60_000,
): ReminderItem[] {
  const out: ReminderItem[] = [];
  const from = toISO(new Date(nowMs - graceMs));
  for (const t of tasks) {
    if (!t.reminders?.length) continue;
    const maxOffset = Math.max(...t.reminders);
    const until = toISO(new Date(nowMs + horizonMs + maxOffset * 60_000));
    const [hh, mm] = (t.time || allDayTime || '09:00').split(':').map(Number);
    for (const o of occurrences(t, from, until, 200)) {
      if (o.done) continue;
      const [y, m, d] = parts(o.start);
      const base = new Date(y, m - 1, d, hh, mm).getTime();
      for (const off of t.reminders) {
        const at = base - off * 60_000;
        if (at <= nowMs - graceMs || at > nowMs + horizonMs) continue;
        const when = `${relativeDay(o.start, toISO(new Date(at)))}${t.time ? ` ${t.time}` : ''}`;
        const notes = t.notes?.split('\n')[0]?.trim();
        out.push({
          // The fire time is part of the key, so moving the task to another time fires again.
          key: `${t.id}@${o.date}#${off}@${at}`,
          atMs: at,
          title: t.title,
          body: [when, off ? offsetLabel(off) : '', notes ?? ''].filter(Boolean).join(' · '),
          taskId: t.id,
          date: o.date,
        });
      }
    }
  }
  return out.sort((a, b) => a.atMs - b.atMs);
}
