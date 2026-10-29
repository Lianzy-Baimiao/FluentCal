import type { ISODate, RepeatRule, Task } from '@/types';
import {
  addDays,
  addMonths,
  dayNum,
  daysInMonth,
  diffDays,
  fromDayNum,
  parts,
  weekday,
  WEEKDAY_SHORT,
  ymd,
} from './date';
import { dayInfo, isLegalWorkday, lunarDayName, lunarMonthName, lunarToSolar } from './lunar';

/** One concrete appearance of a task on the calendar. */
export interface Occurrence {
  task: Task;
  /** Occurrence start (for repeating tasks this identifies the instance). */
  date: ISODate;
  start: ISODate;
  end: ISODate;
  done: boolean;
}

export const EBBINGHAUS_OFFSETS = [0, 1, 2, 4, 7, 15, 30];
const MAX_STEPS = 20_000;

/** Occurrence start dates of a rule, in order, starting at `start`. */
function* starts(rule: RepeatRule, start: ISODate, from: ISODate): Generator<ISODate> {
  const interval = Math.max(1, Math.floor(rule.interval ?? 1));
  const [sy, sm, sd] = parts(start);
  // Jump ahead for unbounded rules so far-future ranges don't iterate from the start.
  const canSkip = rule.count === undefined;
  switch (rule.freq) {
    case 'daily': {
      let k = 0;
      if (canSkip && from > start) k = Math.floor(diffDays(start, from) / interval);
      for (;; k++) yield addDays(start, k * interval);
    }
    case 'weekly': {
      const days = (rule.weekdays?.length ? rule.weekdays : [weekday(start)]).slice().sort((a, b) => a - b);
      // Weeks are counted from the Sunday of the start week.
      const week0 = dayNum(start) - weekday(start);
      let w = 0;
      if (canSkip && from > start) w = Math.max(0, Math.floor((dayNum(from) - week0) / 7 / interval) - 1);
      for (;; w++) {
        for (const wd of days) {
          const n = week0 + w * interval * 7 + wd;
          if (n >= dayNum(start)) yield fromDayNum(n);
        }
      }
    }
    case 'monthly': {
      const nth = Math.ceil(sd / 7); // 1..5, 5 = last
      const wd = weekday(start);
      const lastWeek = sd + 7 > daysInMonth(sy, sm);
      for (let k = 0; ; k++) {
        const [y, m] = parts(addMonths(ymd(sy, sm, 1), k * interval));
        if (rule.monthMode === 'nthWeekday') {
          const firstWd = weekday(ymd(y, m, 1));
          let day = 1 + ((wd - firstWd + 7) % 7) + (Math.min(nth, 4) - 1) * 7;
          if (lastWeek || nth === 5) {
            while (day + 7 <= daysInMonth(y, m)) day += 7;
          }
          yield ymd(y, m, day);
        } else {
          yield ymd(y, m, Math.min(sd, daysInMonth(y, m)));
        }
      }
    }
    case 'yearly': {
      for (let k = 0; ; k++) {
        const y = sy + k * interval;
        yield ymd(y, sm, Math.min(sd, daysInMonth(y, sm)));
      }
    }
    case 'monthLastDay': {
      for (let k = 0; ; k++) {
        const [y, m] = parts(addMonths(ymd(sy, sm, 1), k * interval));
        yield ymd(y, m, daysInMonth(y, m));
      }
    }
    case 'workday':
    case 'legalWorkday': {
      let d = start;
      if (canSkip && from > start) d = from;
      for (;;) {
        const ok = rule.freq === 'workday' ? weekday(d) % 6 !== 0 : isLegalWorkday(d);
        if (ok) yield d;
        d = addDays(d, 1);
      }
    }
    case 'lunarYearly': {
      const info = dayInfo(start);
      for (let k = 0; ; k++) {
        const s = lunarToSolar(info.lunarYear + k * interval, info.lunarMonth, info.lunarDay);
        if (s) yield s;
        if (k > 500) return;
      }
    }
    case 'ebbinghaus': {
      for (const off of EBBINGHAUS_OFFSETS) yield addDays(start, off);
      return;
    }
  }
}

/** Expands a task into occurrences overlapping [rangeStart, rangeEnd] (inclusive). */
export function occurrences(task: Task, rangeStart: ISODate, rangeEnd: ISODate, limit = 400): Occurrence[] {
  const rule = task.repeat;
  if (!rule) {
    if (task.end < rangeStart || task.start > rangeEnd) return [];
    return [{ task, date: task.start, start: task.start, end: task.end, done: task.done }];
  }
  const span = Math.max(0, diffDays(task.start, task.end));
  const from = addDays(rangeStart, -span);
  const ex = new Set(task.exDates ?? []);
  const done = new Set(task.doneDates ?? []);
  const out: Occurrence[] = [];
  let n = 0;
  let steps = 0;
  for (const s of starts(rule, task.start, from)) {
    if (++steps > MAX_STEPS) break;
    if (s < task.start) continue;
    if (rule.until && s > rule.until) break;
    if (s > rangeEnd) break;
    n++;
    if (rule.count !== undefined && n > rule.count) break;
    const e = addDays(s, span);
    if (e < rangeStart || ex.has(s)) continue;
    out.push({ task, date: s, start: s, end: e, done: done.has(s) });
    if (out.length >= limit) break;
  }
  return out;
}

/** Occurrence of a repeating task that starts on `date`, if any. */
export function occurrenceOn(task: Task, date: ISODate): Occurrence | undefined {
  return occurrences(task, date, date).find((o) => o.date === date);
}

/** Next occurrence start on or after `from`. */
export function nextOccurrence(task: Task, from: ISODate): ISODate | undefined {
  const horizon = addDays(from, 800);
  return occurrences(task, from, horizon, 1).find((o) => o.start >= from)?.start;
}

/** Human readable rule, e.g. "每 2 周的周一、周三". */
export function describeRule(rule: RepeatRule | undefined, start: ISODate): string {
  if (!rule) return '不重复';
  const i = Math.max(1, rule.interval ?? 1);
  const every = (unit: string) => (i === 1 ? `每${unit}` : `每 ${i} ${unit}`);
  const [, m, d] = parts(start);
  let s: string;
  switch (rule.freq) {
    case 'daily':
      s = i === 1 ? '每天' : `每 ${i} 天`;
      break;
    case 'weekly': {
      const days = (rule.weekdays?.length ? rule.weekdays : [weekday(start)])
        .slice()
        .sort((a, b) => ((a + 6) % 7) - ((b + 6) % 7))
        .map((x) => `周${WEEKDAY_SHORT[x]}`)
        .join('、');
      s = `${every('周')}的${days}`;
      break;
    }
    case 'monthly':
      if (rule.monthMode === 'nthWeekday') {
        const nth = Math.ceil(d / 7);
        const last = nth >= 5 || d + 7 > daysInMonth(+start.slice(0, 4), m);
        s = `${every('个月')}的${last ? '最后一个' : `第 ${nth} 个`}周${WEEKDAY_SHORT[weekday(start)]}`;
      } else {
        s = `${every('个月')}的 ${d} 日`;
      }
      break;
    case 'yearly':
      s = `${i === 1 ? '每年' : `每 ${i} 年`}的 ${m} 月 ${d} 日`;
      break;
    case 'workday':
      s = '每个工作日（周一至周五）';
      break;
    case 'legalWorkday':
      s = '每个法定工作日（含调休）';
      break;
    case 'lunarYearly': {
      const info = dayInfo(start);
      s = `${i === 1 ? '每年' : `每 ${i} 年`}农历${lunarMonthName(info.lunarMonth)}${lunarDayName(info.lunarDay)}`;
      break;
    }
    case 'monthLastDay':
      s = `${every('个月')}的最后一天`;
      break;
    case 'ebbinghaus':
      return '艾宾浩斯记忆曲线（第 1、2、3、5、8、16、31 天复习）';
  }
  if (rule.count) s += `，共 ${rule.count} 次`;
  else if (rule.until) s += `，直到 ${rule.until}`;
  return s;
}
