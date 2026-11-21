import { describe, expect, it } from 'vitest';
import { doneDay, stampDone } from './done';
import { setSlot } from './flex';
import { mk } from './testing';

// The same cases as native/core/tests/tasks_history.rs (GitHub #15): both sides write the
// same data.
describe('the day a task was ticked done', () => {
  it('is the day of the tick, kept by other edits, gone with the tick', () => {
    const visit = mk({ id: 'visit', start: '2026-11-23', end: '2026-11-25' });
    // ticked two days after it ended: that day, not one of its own
    const ticked = stampDone(visit, { ...visit, done: true, progress: 100 }, '2026-11-27');
    expect(ticked.doneOn).toBe('2026-11-27');
    expect(doneDay(ticked, '2026-11-23')).toBe('2026-11-27');
    // other edits leave it alone (a colour the next day, a move)
    const moved = stampDone(ticked, { ...ticked, color: 'red', start: '2026-11-24', end: '2026-11-26' }, '2026-11-28');
    expect(moved.doneOn).toBe('2026-11-27');
    // corrected by hand (the editor's 完成日期)
    const fixed = stampDone(moved, { ...moved, doneOn: '2026-11-25' }, '2026-11-28');
    expect(fixed.doneOn).toBe('2026-11-25');
    // unticked: gone, not written at all; ticked again: the new day, whatever was set by hand
    const open = stampDone(fixed, { ...fixed, done: false, progress: 0 }, '2026-11-29');
    expect('doneOn' in open).toBe(false);
    expect(doneDay(open, '2026-11-24')).toBeUndefined();
    expect(stampDone(open, { ...open, done: true }, '2026-11-30').doneOn).toBe('2026-11-30');
    // the task that is passed in is never changed
    expect([visit.done, visit.doneOn, fixed.doneOn]).toEqual([false, undefined, '2026-11-25']);
  });

  it('is not made up for a task done before it was recorded', () => {
    const old = mk({ id: 'old', start: '2026-10-01', done: true, progress: 100 });
    const recoloured = stampDone(old, { ...old, color: 'teal' }, '2026-11-27');
    expect('doneOn' in recoloured).toBe(false);
    expect(doneDay(recoloured, '2026-10-01')).toBeUndefined();
    // … until the user says when
    expect(stampDone(old, { ...old, doneOn: '2026-10-02' }, '2026-11-27').doneOn).toBe('2026-10-02');
    // an empty day is no day
    expect('doneOn' in stampDone(old, { ...old, doneOn: '' }, '2026-11-27')).toBe(false);
  });

  it('is today for a new task made done, or the day it names', () => {
    const fresh = mk({ start: '2026-11-20', done: true });
    expect(stampDone(undefined, fresh, '2026-11-28').doneOn).toBe('2026-11-28');
    expect(stampDone(undefined, { ...fresh, doneOn: '2026-11-21' }, '2026-11-28').doneOn).toBe('2026-11-21');
    // a day on a task that isn't done means nothing: dropped
    expect('doneOn' in stampDone(undefined, { ...fresh, done: false, doneOn: '2026-11-21' }, '2026-11-28')).toBe(false);
  });

  it('is a new day after a version that does not know the field unticked the task', () => {
    // (the day that version left behind is stale)
    const stale = mk({ id: 'old', start: '2026-10-01', done: false, doneOn: '2026-10-02' });
    expect(stampDone(stale, { ...stale, done: true, progress: 100 }, '2026-11-29').doneOn).toBe('2026-11-29');
  });

  it('is kept for each time of a repeating task', () => {
    const weekly = mk({ id: 'w', start: '2026-11-02', end: '2026-11-03', repeat: { freq: 'weekly' } });
    const a = stampDone(weekly, { ...weekly, doneDates: ['2026-11-02'] }, '2026-11-04');
    const b = stampDone(a, { ...a, doneDates: ['2026-11-02', '2026-11-09'] }, '2026-11-09');
    expect([doneDay(b, '2026-11-02'), doneDay(b, '2026-11-09'), doneDay(b, '2026-11-16')]).toEqual(['2026-11-04', '2026-11-09', undefined]);
    // the series itself is never done
    expect([b.done, 'doneOn' in b]).toEqual([false, false]);
    // unticking one forgets its day only
    const c = stampDone(b, { ...b, doneDates: ['2026-11-09'] }, '2026-11-11');
    expect(c.doneOnDates).toEqual({ '2026-11-09': '2026-11-09' });
    expect(doneDay(c, '2026-11-02')).toBeUndefined();
    // a time ticked before the days were kept has none, and ticking another leaves it so
    const before = { ...c, doneDates: ['2026-11-09', '2026-11-16'] };
    const d = stampDone(before, { ...before, doneDates: ['2026-11-09', '2026-11-16', '2026-11-23'] }, '2026-11-24');
    expect([doneDay(d, '2026-11-16'), doneDay(d, '2026-11-23')]).toEqual([undefined, '2026-11-24']);
    // a copy (a new task that comes with times done) has them on the days the task has them:
    // a day that isn't known is not made up for the copy either
    const copy = stampDone(undefined, { ...d, id: 'copy' }, '2026-11-25');
    expect([doneDay(copy, '2026-11-09'), doneDay(copy, '2026-11-16'), doneDay(copy, '2026-11-23')]).toEqual(['2026-11-09', undefined, '2026-11-24']);
    // the last one unticked: nothing is left to write
    const none = stampDone(c, { ...c, doneDates: [] }, '2026-11-12');
    expect('doneOnDates' in none).toBe(false);
  });

  it('is the slot record for a to-do without a day', () => {
    const month = mk({ id: 'm', start: '2026-10-01', flex: 'month' });
    const done = setSlot(month, 'm:2026-11', '2026-11-17');
    const saved = stampDone(month, { ...done, doneOn: '2026-11-01', doneOnDates: { x: 'y' } }, '2026-11-17');
    expect(['doneOn' in saved, 'doneOnDates' in saved]).toEqual([false, false]);
    expect(doneDay(saved, 'm:2026-11')).toBe('2026-11-17');
    expect(doneDay(saved, '2026-11-17')).toBe('2026-11-17');
    expect(doneDay(saved, '2026-11-16')).toBeUndefined();
    expect(doneDay(saved, 'm:2026-10')).toBeUndefined();
  });
});
