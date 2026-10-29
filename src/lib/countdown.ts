import type { ISODate, Task } from '@/types';
import { addDays, diffDays } from './date';
import { dayInfo, legalHoliday } from './lunar';
import { occurrences } from './recur';

export interface CountdownItem {
  key: string;
  label: string;
  date: ISODate;
  /** Days from today (0 = today). */
  days: number;
  taskId?: string;
}

/** Festivals that are statutory holidays, used before the official schedule is published. */
const MAJOR = ['元旦', '春节', '清明', '劳动节', '端午节', '中秋节', '国庆节'];

/**
 * First day of the next statutory holiday within ~5 months. When the official schedule for
 * that period is not published yet (e.g. next year's in October), falls back to the date of
 * the next major festival itself.
 */
export function nextHoliday(now: ISODate): { name: string; date: ISODate } | undefined {
  // Skip a holiday that is already running so the countdown moves on to the next one.
  let start = now;
  while (legalHoliday(start)?.work === false) start = addDays(start, 1);
  let d = start;
  for (let i = 0; i < 150; i++, d = addDays(d, 1)) {
    const h = legalHoliday(d);
    if (h && !h.work) return { name: h.name, date: d };
  }
  d = start;
  for (let i = 0; i < 400; i++, d = addDays(d, 1)) {
    const f = dayInfo(d).festivals.find((x) => MAJOR.includes(x));
    if (f && d > now) return { name: f, date: d };
  }
}

/** Countdown entries: tasks flagged as countdown (next occurrence) plus, optionally, the next holiday. */
export function countdowns(tasks: Task[], now: ISODate, withHoliday: boolean): CountdownItem[] {
  const out: CountdownItem[] = [];
  if (withHoliday) {
    const h = nextHoliday(now);
    if (h) out.push({ key: `holiday:${h.date}`, label: h.name, date: h.date, days: diffDays(now, h.date) });
  }
  for (const t of tasks) {
    if (!t.countdown || (t.done && !t.repeat)) continue;
    // The running or next occurrence; one that already started counts as "today".
    const occ = occurrences(t, now, addDays(now, 800), 1)[0];
    if (!occ) continue;
    const date = occ.start < now ? now : occ.start;
    out.push({ key: t.id, label: t.title, date, days: diffDays(now, date), taskId: t.id });
  }
  return out.sort((a, b) => a.days - b.days);
}
