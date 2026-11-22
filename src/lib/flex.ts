// 不定日待办 (GitHub #7): things to do without a day. The same rules as the desktop's
// native/core/src/flex.rs, so both sides read and write the same data:
// - "month": once every month, on whichever day. Each month has one slot ("m:2026-11");
//   ticking it records the day it was done. Last month's slot left open is overdue.
// - "someday": once, no date at all (slot "s:"); gone from the list when done.
// `flexDone` maps a slot's key ("2026-11", or "" for someday) to the day it was done.
import type { ISODate, Task } from '@/types';
import { addMonths, endOfMonth, isISODate, startOfMonth } from './date';
import type { Occurrence } from './recur';

export const MONTH = 'month';
export const SOMEDAY = 'someday';
export const SOMEDAY_SLOT = 's:';

/** "month" | "someday"; a kind written by a newer version reads as none. */
export function flexKind(t: Task): 'month' | 'someday' | undefined {
  return t.flex === MONTH || t.flex === SOMEDAY ? t.flex : undefined;
}

export const monthSlot = (date: ISODate) => `m:${date.slice(0, 7)}`;

/** A slot ("m:2026-11", "s:") rather than a calendar date. */
export const isSlot = (date: string) => date.startsWith('m:') || date === SOMEDAY_SLOT;

const keyOf = (slot: string) => (slot.includes(':') ? slot.slice(slot.indexOf(':') + 1) : slot);

/** The day a slot was done. */
export function slotDone(t: Task, slot: string): ISODate | undefined {
  return t.flexDone?.[keyOf(slot)];
}

/** Ticks a slot (done on `day`) or unticks it. */
export function setSlot(t: Task, slot: string, day: ISODate | undefined): Task {
  const m = { ...(t.flexDone ?? {}) };
  if (day) m[keyOf(slot)] = day;
  else delete m[keyOf(slot)];
  const any = Object.keys(m).length > 0;
  const out: Task = { ...t, flexDone: any ? m : undefined };
  if (flexKind(t) === SOMEDAY) {
    out.done = any;
    out.progress = any ? 100 : 0;
  }
  return out;
}

/** Unticks what was done on `day` (the done entry on that day was unticked). */
export function clearDay(t: Task, day: ISODate): Task {
  let out = t;
  for (const [k, d] of Object.entries(t.flexDone ?? {})) if (d === day) out = setSlot(out, k, undefined);
  return out;
}

/** Every calendar occurrence created by a completed flex slot, deduplicated by date. */
export function doneOccurrences(t: Task): Occurrence[] {
  if (!flexKind(t)) return [];
  return [...new Set(Object.values(t.flexDone ?? {}).filter(isISODate))]
    .sort()
    .map((date) => ({ task: t, date, start: date, end: date, done: true }));
}

export interface FlexSide {
  /** This month's, with the day each was done. Done ones stay. */
  month: { task: Task; done?: ISODate }[];
  /** Left open last month. */
  overdue: Task[];
  /** Without any date, not done. */
  someday: Task[];
  monthSlot: string;
  overdueSlot: string;
}

/** The to-do list without days, as on `today`. */
export function flexSide(tasks: Task[], today: ISODate): FlexSide {
  const cur = today.slice(0, 7);
  const prev = addMonths(startOfMonth(today), -1).slice(0, 7);
  const s: FlexSide = { month: [], overdue: [], someday: [], monthSlot: `m:${cur}`, overdueSlot: `m:${prev}` };
  for (const t of tasks) {
    const kind = flexKind(t);
    const first = t.start.slice(0, 7);
    if (kind === MONTH) {
      if (first <= cur) s.month.push({ task: t, done: slotDone(t, s.monthSlot) });
      if (first <= prev && !slotDone(t, s.overdueSlot)) s.overdue.push(t);
    } else if (kind === SOMEDAY && !slotDone(t, SOMEDAY_SLOT)) {
      s.someday.push(t);
    }
  }
  const byOrder = (a: Task, b: Task) => a.order - b.order || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
  s.month.sort((a, b) => byOrder(a.task, b.task));
  s.overdue.sort(byOrder);
  s.someday.sort(byOrder);
  return s;
}

/** This month's slot of a "month" task while it is open: its bar in the Gantt chart. */
export function openMonth(t: Task, today: ISODate): { slot: string; start: ISODate; end: ISODate } | undefined {
  if (flexKind(t) !== MONTH || t.start.slice(0, 7) > today.slice(0, 7)) return undefined;
  const slot = monthSlot(today);
  return slotDone(t, slot) ? undefined : { slot, start: startOfMonth(today), end: endOfMonth(today) };
}

/**
 * What reminders count from: the last day of each month in the range, for a "month" task
 * still open in that month ("以月底为基准，提前 N 天提醒").
 */
export function dueOccurrences(t: Task, rangeStart: ISODate, rangeEnd: ISODate): Occurrence[] {
  if (flexKind(t) !== MONTH) return [];
  const out: Occurrence[] = [];
  for (let first = startOfMonth(rangeStart); first <= rangeEnd; first = addMonths(first, 1)) {
    const last = endOfMonth(first);
    const slot = monthSlot(first);
    if (last >= rangeStart && last <= rangeEnd && t.start.slice(0, 7) <= first.slice(0, 7) && !slotDone(t, slot)) {
      out.push({ task: t, date: slot, start: last, end: last, done: false });
    }
  }
  return out;
}

/** The fields of a new to-do written into the list on `today`. */
export function newFlex(kind: 'month' | 'someday', title: string, today: ISODate) {
  const start = kind === MONTH ? startOfMonth(today) : today;
  return { title, start, end: start, flex: kind, autoPostpone: false, reminders: undefined, color: undefined };
}

/** Whether a task still has something to do today (the "N 项待完成" count). */
export function flexOpen(t: Task, today: ISODate): boolean {
  const kind = flexKind(t);
  if (kind === SOMEDAY) return !slotDone(t, SOMEDAY_SLOT);
  if (kind === MONTH) return t.start.slice(0, 7) <= today.slice(0, 7) && !slotDone(t, monthSlot(today));
  return false;
}
