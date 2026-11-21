import type { Group, ISODate, Task } from '@/types';
import { doc, put } from '@/stores/doc';
import { addDays, diffDays, today } from './date';
import type { Occurrence } from './recur';
import { clearDay, flexKind, isSlot, setSlot } from './flex';
import { doneDay, stampDone } from './done';

import { uid } from './uid';

export { doneDay, stampDone, uid };

function nextOrder(): number {
  let max = 0;
  for (const t of Object.values(doc.tasks)) if (t.order > max) max = t.order;
  return max + 1;
}

/** New task with defaults; `p` overrides everything except id/order/timestamps. */
export function makeTask(p: Partial<Task> & { title: string; start: ISODate }): Task {
  const now = Date.now();
  const offsets = doc.settings.reminders.defaultOffsets;
  // made done (the editor's 已完成 box): done today
  return stampDone(undefined, {
    end: p.start,
    progress: 0,
    done: false,
    color: doc.settings.tasks.defaultColor || undefined,
    reminders: offsets.length ? [...offsets] : undefined,
    ...p,
    id: uid(),
    order: p.order ?? nextOrder(),
    createdAt: now,
    updatedAt: now,
  }, today());
}

export function saveTask(t: Task): Promise<void> {
  const old = doc.tasks[t.id];
  // A renamed sample task is the user's own now: "清除示例数据" must not take it.
  const own = t.sample && old && old.title !== t.title;
  return put('tasks', { [t.id]: { ...stampDone(old, t, today()), sample: own ? undefined : t.sample, updatedAt: Date.now() } });
}

export function addTask(p: Partial<Task> & { title: string; start: ISODate }): Promise<Task> {
  const t = makeTask(p);
  return put('tasks', { [t.id]: t }).then(() => t);
}

/** Deletes a task and removes it from other tasks' predecessors. */
export function deleteTask(id: string): Promise<void> {
  const items: Record<string, Task | null> = { [id]: null };
  for (const t of Object.values(doc.tasks)) {
    if (t.deps?.includes(id)) items[t.id] = { ...t, deps: t.deps.filter((d) => d !== id), updatedAt: Date.now() };
  }
  return put('tasks', items);
}

/** `date`: the occurrence; for a to-do without a day (flex.ts) its slot (done today), or the
 * day it was done (to untick it there). */
export function setOccurrenceDone(t: Task, date: ISODate, done: boolean): Promise<void> {
  if (flexKind(t)) {
    if (isSlot(date)) return saveTask(setSlot(t, date, done ? today() : undefined));
    return done ? Promise.resolve() : saveTask(clearDay(t, date));
  }
  if (t.repeat) {
    const set = new Set(t.doneDates ?? []);
    if (done) set.add(date);
    else set.delete(date);
    return saveTask({ ...t, doneDates: [...set].sort() });
  }
  return saveTask({ ...t, done, progress: done ? 100 : t.progress === 100 ? 0 : t.progress });
}

export function setProgress(t: Task, progress: number): Promise<void> {
  const p = Math.max(0, Math.min(100, Math.round(progress)));
  return saveTask({ ...t, progress: p, done: t.repeat ? t.done : p >= 100 });
}

export function setColor(t: Task, color: string): Promise<void> {
  return saveTask({ ...t, color: color || undefined });
}

/** Moves a task (keeping its duration) so it starts on `start`. */
export function moveTask(t: Task, start: ISODate): Promise<void> {
  return saveTask({ ...t, start, end: addDays(start, diffDays(t.start, t.end)) });
}

/**
 * Moves one occurrence. For a repeating task the occurrence is detached: it is excluded from
 * the series and a standalone copy is created on the new date.
 */
export function moveOccurrence(occ: Occurrence, start: ISODate): Promise<void> {
  const t = occ.task;
  if (!t.repeat) return moveTask(t, start);
  if (start === occ.start) return Promise.resolve();
  const copy = makeTask({
    ...t,
    repeat: undefined,
    doneDates: undefined,
    exDates: undefined,
    start,
    end: addDays(start, diffDays(occ.start, occ.end)),
    done: occ.done,
    progress: occ.done ? 100 : 0,
  });
  // done on the day the occurrence was (not known: not made up)
  const day = doneDay(t, occ.date);
  if (day) copy.doneOn = day;
  else delete copy.doneOn;
  const series = { ...t, exDates: [...new Set([...(t.exDates ?? []), occ.date])].sort(), updatedAt: Date.now() };
  return put('tasks', { [t.id]: series, [copy.id]: copy });
}

export type DeleteMode = 'this' | 'future' | 'all';

export function deleteOccurrence(t: Task, date: ISODate, mode: DeleteMode): Promise<void> {
  if (!t.repeat || mode === 'all' || (mode === 'future' && date <= t.start)) return deleteTask(t.id);
  if (mode === 'this') return saveTask({ ...t, exDates: [...new Set([...(t.exDates ?? []), date])].sort() });
  return saveTask({ ...t, repeat: { ...t.repeat, count: undefined, until: addDays(date, -1) } });
}

export function duplicateTask(t: Task): Promise<Task> {
  const copy = makeTask({ ...t, title: `${t.title}（副本）`, order: undefined, sample: undefined });
  // a copy of a done task was done when the task was (not known: not made up)
  if (copy.done && t.doneOn) copy.doneOn = t.doneOn;
  else delete copy.doneOn;
  return put('tasks', { [copy.id]: copy }).then(() => copy);
}

/** Moves overdue single-day tasks (late by at most 30 days) to today. */
export function autoPostpone(now: ISODate = today()): Promise<void> | undefined {
  const items: Record<string, Task> = {};
  for (const t of Object.values(doc.tasks)) {
    if (t.done || t.repeat || t.flex || t.start !== t.end) continue;
    if (!(t.autoPostpone ?? doc.settings.tasks.autoPostpone)) continue;
    const late = diffDays(t.end, now);
    if (late <= 0 || late > 30) continue;
    items[t.id] = { ...t, start: now, end: now, updatedAt: Date.now() };
  }
  return Object.keys(items).length ? put('tasks', items, { history: false }) : undefined;
}

export function makeGroup(name: string, color = 'blue'): Group {
  const order = Math.max(0, ...Object.values(doc.groups).map((g) => g.order)) + 1;
  return { id: uid(), name, color, order };
}

export function saveGroup(g: Group): Promise<void> {
  const own = g.sample && doc.groups[g.id] && doc.groups[g.id].name !== g.name;
  return put('groups', { [g.id]: own ? { ...g, sample: undefined } : g });
}

/** First-run sample tasks and groups still there. */
export function sampleCount(): number {
  return Object.values(doc.tasks).filter((t) => t.sample).length + Object.values(doc.groups).filter((g) => g.sample).length;
}

/** "清除示例数据": removes the sample tasks and groups in one undo step. The user's own tasks
 * in a sample group become ungrouped; predecessors pointing at sample tasks are dropped. */
export async function clearSampleData(): Promise<number> {
  const gone = new Set(Object.values(doc.tasks).filter((t) => t.sample).map((t) => t.id));
  const groups = Object.values(doc.groups).filter((g) => g.sample).map((g) => g.id);
  const tasks: Record<string, Task | null> = {};
  for (const id of gone) tasks[id] = null;
  for (const t of Object.values(doc.tasks)) {
    if (gone.has(t.id)) continue;
    const ungroup = !!t.groupId && groups.includes(t.groupId);
    const deps = t.deps?.some((d) => gone.has(d));
    if (ungroup || deps) tasks[t.id] = { ...t, groupId: ungroup ? undefined : t.groupId, deps: deps ? t.deps!.filter((d) => !gone.has(d)) : t.deps, updatedAt: Date.now() };
  }
  if (Object.keys(tasks).length) await put('tasks', tasks);
  if (groups.length) await put('groups', Object.fromEntries(groups.map((id) => [id, null])));
  return gone.size;
}

/** Deletes a group; its tasks become ungrouped. */
export async function deleteGroup(id: string): Promise<void> {
  const items: Record<string, Task> = {};
  for (const t of Object.values(doc.tasks)) {
    if (t.groupId === id) items[t.id] = { ...t, groupId: undefined, updatedAt: Date.now() };
  }
  if (Object.keys(items).length) await put('tasks', items);
  await put('groups', { [id]: null });
}

export function setDayMark(date: ISODate, color: string | null): Promise<void> {
  return put('dayMarks', { [date]: color ? { color } : null });
}
