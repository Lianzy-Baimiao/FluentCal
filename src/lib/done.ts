import type { ISODate, Task } from '@/types';
import { isISODate } from './date';
import { flexKind, isSlot, slotDone } from './flex';

// The day a task was ticked done (GitHub #15). Pure rules, shared by the web view and the
// Android app; lib/tasks.ts applies them to everything that is saved.

/**
 * Keeps the day a task was ticked done (GitHub #15) in step with its done state, whoever
 * writes it. The same rule as the desktop's (native/core/src/tasks.rs `stamp_done`), so both
 * sides agree on the synced data:
 * - ticked: `today`, unless this very edit names the day (the editor's 完成日期);
 * - unticked: no day, so ticking it again is a new day;
 * - done before and after: the day as the edit has it. A task done before the day was
 *   recorded has none, and none is made up for it.
 *
 * Repeating tasks keep a day per occurrence, the same way. To-dos without a day have their
 * own record (`flexDone`). `old`: the task as stored (undefined: a new one).
 */
export function stampDone(old: Task | undefined, task: Task, today: ISODate): Task {
  const t: Task = { ...task };
  const given = t.doneOnDates;
  delete t.doneOnDates;
  if (!t.doneOn) delete t.doneOn;
  if (flexKind(t)) {
    delete t.doneOn;
    return t;
  }
  if (t.repeat) {
    const done = t.doneDates ?? [];
    const days: Record<ISODate, ISODate> = {};
    for (const [k, v] of Object.entries(given ?? {})) if (done.includes(k)) days[k] = v;
    // (a new task that comes with times done is a copy: no day is made up for it)
    if (old) {
      const was = new Set(old.doneDates ?? []);
      for (const d of done) if (!was.has(d) && !days[d]) days[d] = today;
    }
    if (Object.keys(days).length) t.doneOnDates = days;
  }
  // (a repeating task keeps `done` and its day for the day it stops repeating)
  if (!t.done) {
    delete t.doneOn;
    return t;
  }
  const named = !!t.doneOn && (!old || old.doneOn !== t.doneOn);
  if (!old?.done && !named) t.doneOn = today;
  return t;
}

/**
 * The day an occurrence was ticked done, when it is done and the day is known. `date` as in
 * `setOccurrenceDone`: the occurrence, or a to-do's slot or the day it was done.
 */
export function doneDay(t: Task, date: string): ISODate | undefined {
  if (flexKind(t)) return isSlot(date) ? slotDone(t, date) : Object.values(t.flexDone ?? {}).find((d) => d === date);
  // (what is shown is a day: a file written by hand may hold anything)
  const day = t.repeat ? (t.doneDates?.includes(date) ? t.doneOnDates?.[date] : undefined) : t.done ? t.doneOn : undefined;
  return isISODate(day) ? day : undefined;
}
