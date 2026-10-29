import type { Group, ISODate, Task } from '@/types';
import { doc, put } from '@/stores/doc';
import { addDays, diffDays, today } from './date';
import type { Occurrence } from './recur';

import { uid } from './uid';

export { uid };

function nextOrder(): number {
  let max = 0;
  for (const t of Object.values(doc.tasks)) if (t.order > max) max = t.order;
  return max + 1;
}

/** New task with defaults; `p` overrides everything except id/order/timestamps. */
export function makeTask(p: Partial<Task> & { title: string; start: ISODate }): Task {
  const now = Date.now();
  const offsets = doc.settings.reminders.defaultOffsets;
  return {
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
  };
}

export function saveTask(t: Task): Promise<void> {
  return put('tasks', { [t.id]: { ...t, updatedAt: Date.now() } });
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

export function setOccurrenceDone(t: Task, date: ISODate, done: boolean): Promise<void> {
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
  return addTask({ ...t, title: `${t.title}（副本）`, order: undefined });
}

/** Moves overdue single-day tasks (late by at most 30 days) to today. */
export function autoPostpone(now: ISODate = today()): Promise<void> | undefined {
  const items: Record<string, Task> = {};
  for (const t of Object.values(doc.tasks)) {
    if (t.done || t.repeat || t.start !== t.end) continue;
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
  return put('groups', { [g.id]: g });
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
