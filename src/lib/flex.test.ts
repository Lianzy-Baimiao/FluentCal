import { describe, expect, it } from 'vitest';
import { clearDay, doneOccurrences, dueOccurrences, flexOpen, flexSide, isSlot, newFlex, openMonth, setSlot, slotDone } from './flex';
import { occurrences } from './recur';
import { computeReminders } from './reminders';
import { mk } from './testing';

const month = (p = {}) => mk({ id: 'm', title: '洗车', start: '2026-10-01', flex: 'month', ...p });
const someday = (p = {}) => mk({ id: 's', title: '整理相册', start: '2026-11-08', flex: 'someday', ...p });

describe('to-dos without a day', () => {
  it('ticks a month slot on the day it was done and shows it there on the calendar', () => {
    const t = setSlot(month(), 'm:2026-11', '2026-11-17');
    expect(t.flexDone).toEqual({ '2026-11': '2026-11-17' });
    expect(t.done).toBe(false); // next month it is open again
    expect(slotDone(t, 'm:2026-11')).toBe('2026-11-17');
    expect(occurrences(t, '2026-11-01', '2026-11-30').map((o) => [o.date, o.done])).toEqual([['2026-11-17', true]]);
    // an open one is on no day
    expect(occurrences(month(), '2026-10-01', '2026-12-31')).toEqual([]);
  });

  it('unticks by slot or by the day', () => {
    const t = setSlot(month(), 'm:2026-11', '2026-11-17');
    expect(setSlot(t, 'm:2026-11', undefined).flexDone).toBeUndefined();
    expect(clearDay(t, '2026-11-17').flexDone).toBeUndefined();
    expect(clearDay(t, '2026-11-16').flexDone).toEqual({ '2026-11': '2026-11-17' });
  });

  it('builds unique completed occurrences and ignores invalid stored dates', () => {
    const t = month({ flexDone: { '2026-10': '2026-11-17', '2026-11': '2026-11-17', bad: 'not-a-date' } });
    expect(doneOccurrences(t).map((o) => [o.date, o.done])).toEqual([['2026-11-17', true]]);
  });

  it('marks a someday to-do done as a whole', () => {
    const t = setSlot(someday(), 's:', '2026-11-17');
    expect([t.done, t.progress, t.flexDone]).toEqual([true, 100, { '': '2026-11-17' }]);
    const back = setSlot(t, 's:', undefined);
    expect([back.done, back.progress, back.flexDone]).toEqual([false, 0, undefined]);
  });

  it('lists this month, what last month left open, and the ones without a date', () => {
    const done = setSlot(month({ id: 'a', order: 1 }), 'm:2026-11', '2026-11-03');
    const open = month({ id: 'b', order: 2, title: '交水电费' });
    const fresh = month({ id: 'c', order: 3, start: '2026-11-01' }); // first month is this one: not overdue
    const later = month({ id: 'd', order: 4, start: '2026-12-01' });
    const paidOct = setSlot(month({ id: 'e', order: 5 }), 'm:2026-10', '2026-10-20');
    const gone = setSlot(someday({ id: 'g' }), 's:', '2026-11-10');
    const s = flexSide([paidOct, later, fresh, open, done, someday({ id: 'f' }), gone, mk({ id: 'plain', start: '2026-11-17' })], '2026-11-17');
    expect(s.month.map((x) => [x.task.id, x.done])).toEqual([['a', '2026-11-03'], ['b', undefined], ['c', undefined], ['e', undefined]]);
    expect(s.overdue.map((t) => t.id)).toEqual(['a', 'b']);
    expect(s.someday.map((t) => t.id)).toEqual(['f']);
    expect([s.monthSlot, s.overdueSlot]).toEqual(['m:2026-11', 'm:2026-10']);
  });

  it('crosses the year for last month', () => {
    const s = flexSide([month({ start: '2026-12-01' })], '2027-01-05');
    expect(s.overdueSlot).toBe('m:2026-12');
    expect(s.overdue.length).toBe(1);
  });

  it('counts what is still to do', () => {
    expect(flexOpen(month(), '2026-11-17')).toBe(true);
    expect(flexOpen(setSlot(month(), 'm:2026-11', '2026-11-02'), '2026-11-17')).toBe(false);
    expect(flexOpen(month({ start: '2026-12-01' }), '2026-11-17')).toBe(false);
    expect(flexOpen(someday(), '2026-11-17')).toBe(true);
    expect(flexOpen(mk({ start: '2026-11-17' }), '2026-11-17')).toBe(false);
  });

  it('is a bar over the month while open', () => {
    expect(openMonth(month(), '2026-11-17')).toEqual({ slot: 'm:2026-11', start: '2026-11-01', end: '2026-11-30' });
    expect(openMonth(setSlot(month(), 'm:2026-11', '2026-11-02'), '2026-11-17')).toBeUndefined();
    expect(openMonth(someday(), '2026-11-17')).toBeUndefined();
  });

  it('reminds from the end of the month while open', () => {
    const t = month({ reminders: [1440] });
    expect(dueOccurrences(t, '2026-11-17', '2027-01-10').map((o) => [o.date, o.start])).toEqual([['m:2026-11', '2026-11-30'], ['m:2026-12', '2026-12-31']]);
    expect(dueOccurrences(setSlot(t, 'm:2026-11', '2026-11-20'), '2026-11-17', '2026-12-10')).toEqual([]);
    const now = new Date(2026, 10, 28, 8, 0).getTime();
    const items = computeReminders([t], now, '09:00');
    expect(items.map((x) => [x.date, new Date(x.atMs).getDate(), new Date(x.atMs).getHours()])).toEqual([['m:2026-11', 29, 9]]);
    expect(computeReminders([setSlot(t, 'm:2026-11', '2026-11-20')], now, '09:00')).toEqual([]);
  });

  it('writes a new to-do the way the desktop does', () => {
    expect(newFlex('month', '洗车', '2026-11-17')).toMatchObject({ start: '2026-11-01', end: '2026-11-01', flex: 'month', autoPostpone: false });
    expect(newFlex('someday', '换护照', '2026-11-17')).toMatchObject({ start: '2026-11-17', flex: 'someday' });
    expect([isSlot('m:2026-11'), isSlot('s:'), isSlot('2026-11-17')]).toEqual([true, true, false]);
  });
});

describe('once a day over a range (GitHub #12)', () => {
  it('is one day each, up to the end date', () => {
    const t = mk({ start: '2026-11-14', end: '2026-11-20', repeat: { freq: 'daily' } });
    const o = occurrences(t, '2026-11-01', '2026-11-30');
    expect(o.map((x) => x.date)).toEqual(['2026-11-14', '2026-11-15', '2026-11-16', '2026-11-17', '2026-11-18', '2026-11-19', '2026-11-20']);
    expect(o.every((x) => x.start === x.end)).toBe(true);
  });

  it('every other day stops on the end date too', () => {
    const t = mk({ start: '2026-11-14', end: '2026-11-20', repeat: { freq: 'daily', interval: 2 } });
    expect(occurrences(t, '2026-11-01', '2026-11-30').map((x) => x.date)).toEqual(['2026-11-14', '2026-11-16', '2026-11-18', '2026-11-20']);
  });
});
