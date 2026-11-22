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
const festivalHolidaySkip = (name: string) => MAJOR.some(holiday => name === holiday || name.includes(holiday));

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

/** Countdown entries from manually marked tasks, selected groups, and opt-in observances. */
export function countdowns(tasks: Task[], now: ISODate, withHoliday: boolean, marked = true, groups: readonly string[] = [], withFestivals = false, withTerms = false): CountdownItem[] {
  const out: CountdownItem[] = [];
  if (withHoliday) {
    const h = nextHoliday(now);
    if (h) out.push({ key: `holiday:${h.date}`, label: h.name, date: h.date, days: diffDays(now, h.date) });
  }
  if (withHoliday && !withFestivals && !withTerms) return out;
  for (let offset = 1; offset <= 366; offset++) {
    const date = addDays(now, offset);
    const info = dayInfo(date);
    const holiday = withHoliday ? legalHoliday(date) : undefined;
    const observances: Array<[string, string]> = [];
    const previousHoliday = holiday?.work === false ? legalHoliday(addDays(date, -1)) : undefined;
    const startsHoliday = !previousHoliday || previousHoliday.work || previousHoliday.name !== holiday?.name;
    if (holiday?.work === false && startsHoliday) observances.push(['holiday', holiday.name]);
    if (withFestivals) observances.push(...info.festivals.filter(name => !festivalHolidaySkip(name)).map(name => ['festival', name] as [string, string]));
    if (withTerms && info.term) observances.push(['term', info.term]);
    for (const [prefix, name] of observances) {
      const sameDate = out.find(item => item.date === date && !item.taskId);
      if (sameDate) {
        const normalized = (text: string) => text.replaceAll('节假日', '').replaceAll('节', '');
        if (!sameDate.label.split('、').some(part => normalized(part) === normalized(name))) sameDate.label = `${sameDate.label}、${name}`;
      } else out.push({ key: `${prefix}:${date}`, label: name, date, days: offset });
    }
  }
  for (const t of tasks) {
    if (!(marked && t.countdown || groups.includes(t.groupId ?? '')) || (t.done && !t.repeat)) continue;
    // The running or next occurrence; one that already started counts as "today".
    const occ = occurrences(t, now, addDays(now, 800), 1)[0];
    if (!occ) continue;
    const date = occ.start < now ? now : occ.start;
    out.push({ key: t.id, label: t.title, date, days: diffDays(now, date), taskId: t.id });
  }
  return out.sort((a, b) => a.days - b.days);
}
