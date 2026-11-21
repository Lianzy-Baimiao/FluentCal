import { beforeAll, describe, expect, it } from 'vitest';
import { today } from './date';
import { occurrenceOn } from './recur';

// tasks.ts writes through the store, which talks to the browser mock in api.ts; give it an
// in-memory localStorage (as stores/doc.test.ts does).
const mem = new Map<string, string>();
(globalThis as Record<string, unknown>).localStorage = {
  getItem: (k: string) => mem.get(k) ?? null,
  setItem: (k: string, v: string) => void mem.set(k, v),
  removeItem: (k: string) => void mem.delete(k),
};

let store: typeof import('@/stores/doc');
let m: typeof import('./tasks');
beforeAll(async () => {
  store = await import('@/stores/doc');
  m = await import('./tasks');
  await store.initStore();
});

const get = (id: string) => store.doc.tasks[id];

// Everything that saves a task keeps the day it was ticked done (GitHub #15), as the desktop does.
describe('saving keeps the completion day', () => {
  it('stamps a tick, clears an untick, and leaves other edits alone', async () => {
    const t = await m.addTask({ title: '看望小猫', start: '2026-11-23', end: '2026-11-25' });
    expect('doneOn' in get(t.id)).toBe(false);
    await m.setOccurrenceDone(get(t.id), t.start, true);
    expect([get(t.id).done, get(t.id).doneOn]).toEqual([true, today()]);
    // corrected by hand, then a move and a colour: the day stays
    await m.saveTask({ ...get(t.id), doneOn: '2026-11-25' });
    await m.moveTask(get(t.id), '2026-11-24');
    await m.setColor(get(t.id), 'red');
    expect([get(t.id).start, get(t.id).doneOn]).toEqual(['2026-11-24', '2026-11-25']);
    // progress below 100 % is an untick, 100 % a tick
    await m.setProgress(get(t.id), 60);
    expect([get(t.id).done, 'doneOn' in get(t.id)]).toEqual([false, false]);
    await m.setProgress(get(t.id), 100);
    expect(get(t.id).doneOn).toBe(today());
    await m.setOccurrenceDone(get(t.id), get(t.id).start, false);
    expect('doneOn' in get(t.id)).toBe(false);
  });

  it('makes a new task that is done, done today', async () => {
    const done = await m.addTask({ title: '已办', start: '2026-11-20', done: true, progress: 100 });
    expect(get(done.id).doneOn).toBe(today());
    const named = await m.addTask({ title: '补记', start: '2026-11-20', done: true, doneOn: '2026-11-21' });
    expect(get(named.id).doneOn).toBe('2026-11-21');
  });

  it('copies a done task with the day it was done, never a made-up one', async () => {
    const t = await m.addTask({ title: '交房租', start: '2026-10-01', done: true, doneOn: '2026-10-02' });
    const copy = await m.duplicateTask(get(t.id));
    expect([copy.done, get(copy.id).doneOn]).toEqual([true, '2026-10-02']);
    // done before the day was recorded
    await store.put('tasks', { old: { id: 'old', title: '旧任务', start: '2026-09-01', end: '2026-09-01', progress: 100, done: true, order: 1, createdAt: 0, updatedAt: 0 } });
    const copy2 = await m.duplicateTask(get('old'));
    expect([copy2.done, 'doneOn' in get(copy2.id)]).toEqual([true, false]);
  });

  it('keeps the day of each time of a repeating task, and hands it to the one dragged out', async () => {
    const w = await m.addTask({ title: '周会', start: '2026-11-02', repeat: { freq: 'weekly' } });
    await m.setOccurrenceDone(get(w.id), '2026-11-09', true);
    expect(get(w.id).doneOnDates).toEqual({ '2026-11-09': today() });
    expect(m.doneDay(get(w.id), '2026-11-09')).toBe(today());
    // by hand, as the desktop's 修改完成日期 writes it
    await m.saveTask({ ...get(w.id), doneOnDates: { '2026-11-09': '2026-11-10' } });
    // a time ticked by a version that does not know the field has no day
    await store.put('tasks', { [w.id]: { ...get(w.id), doneDates: ['2026-11-09', '2026-11-16'] } });
    await m.setOccurrenceDone(get(w.id), '2026-11-23', true);
    expect(get(w.id).doneOnDates).toEqual({ '2026-11-09': '2026-11-10', '2026-11-23': today() });
    // dragged out of the series: the task it becomes was done when that time was
    const before = new Set(Object.keys(store.doc.tasks));
    await m.moveOccurrence(occurrenceOn(get(w.id), '2026-11-09')!, '2026-11-12');
    const one = Object.values(store.doc.tasks).find((t) => !before.has(t.id))!;
    expect([one.start, one.done, one.doneOn, 'doneOnDates' in one, one.repeat]).toEqual(['2026-11-12', true, '2026-11-10', false, undefined]);
    expect(get(w.id).exDates).toEqual(['2026-11-09']);
    // … and not known for the time: not made up for the task
    const before2 = new Set(Object.keys(store.doc.tasks));
    await m.moveOccurrence(occurrenceOn(get(w.id), '2026-11-16')!, '2026-11-18');
    const two = Object.values(store.doc.tasks).find((t) => !before2.has(t.id))!;
    expect([two.done, 'doneOn' in two]).toEqual([true, false]);
    // unticking one forgets its day only (the series still has the time that left it as done)
    await m.setOccurrenceDone(get(w.id), '2026-11-23', false);
    expect(get(w.id).doneOnDates).toEqual({ '2026-11-09': '2026-11-10' });
  });
});
