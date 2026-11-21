import { describe, expect, it } from 'vitest';
import { barLabel, doneShort, occDoneDay } from './bar';
import { addDays } from './date';
import { occurrences } from './recur';
import { mk } from './testing';

const today = '2026-11-20';
const all = { days: true, progress: true, done: true };
const occ = (t: ReturnType<typeof mk>, on = t.start) => occurrences(t, on, on)[0];
/** The label of the whole bar (one piece from its first day). */
const label = (t: ReturnType<typeof mk>, show = all) => {
  const o = occ(t, t.start < today ? today : t.start) ?? occurrences(t, t.start, t.end)[0];
  return barLabel(o, o.start, 7, today, show).text;
};

// The same texts as the desktop's bars (native/app/tests/e2e.rs `bars_say_which_day_it_is`).
describe('what a bar says after its title', () => {
  const cat = mk({ title: '看望小猫', start: addDays(today, -1), end: addDays(today, 3), progress: 60 });

  it('counts the days from today and names the progress', () => {
    expect(label(cat)).toBe('第5-2天（40%） 进度 60%');
    expect(label(mk({ start: addDays(today, 2), end: addDays(today, 4) }))).toBe('第3-0天（0%） 进度 0%');
    const over = mk({ start: addDays(today, -4), end: addDays(today, -2), done: true, progress: 100 });
    expect(barLabel(occurrences(over, over.start, over.end)[0], over.start, 3, today, all).text).toBe('第3-3天（100%） 进度 100%');
  });

  it('says nothing on one day or a milestone', () => {
    expect(label(mk({ start: today }))).toBe('');
    expect(label(mk({ start: today, end: addDays(today, 2), milestone: true }))).toBe('');
  });

  it('has days but no progress for a repeating task', () => {
    const camp = mk({ start: addDays(today, -1), end: today, repeat: { freq: 'monthly', interval: 1 } });
    const o = occurrences(camp, today, today)[0];
    expect(barLabel(o, o.start, 2, today, all).text).toBe('第2-2天（100%）');
  });

  it('follows the three switches', () => {
    expect(label(cat, { ...all, progress: false })).toBe('第5-2天（40%）');
    expect(label(cat, { ...all, days: false })).toBe('进度 60%');
    expect(label(cat, { days: false, progress: false, done: true })).toBe('');
  });

  it('names the day it was ticked done in place of the progress', () => {
    const done = mk({ start: '2026-11-23', end: '2026-11-25', done: true, progress: 100, doneOn: '2026-11-27' });
    const o = occurrences(done, done.start, done.end)[0];
    expect(occDoneDay(o)).toBe('2026-11-27');
    expect(barLabel(o, o.start, 3, '2026-11-28', all).text).toBe('第3-3天（100%） 26.11.27完成');
    expect(barLabel(o, o.start, 3, '2026-11-28', { ...all, days: false }).text).toBe('26.11.27完成');
    // the switch off: the progress as before
    expect(barLabel(o, o.start, 3, '2026-11-28', { ...all, done: false }).text).toBe('第3-3天（100%） 进度 100%');
    expect(barLabel(o, o.start, 3, '2026-11-28', { days: false, progress: false, done: true }).text).toBe('26.11.27完成');
    expect(doneShort('2005-01-09')).toBe('05.1.9完成');
  });

  it('names the day of this time of a repeating task', () => {
    const camp = mk({ start: '2026-11-02', end: '2026-11-03', repeat: { freq: 'weekly' }, doneDates: ['2026-11-09'], doneOnDates: { '2026-11-09': '2026-11-11' } });
    const [first, second] = occurrences(camp, '2026-11-02', '2026-11-10');
    expect(barLabel(first, first.start, 2, '2026-11-12', all).text).toBe('第2-2天（100%）');
    expect(barLabel(second, second.start, 2, '2026-11-12', all).text).toBe('第2-2天（100%） 26.11.11完成');
  });

  it('has no day for a to-do without one (it sits on the day it was done)', () => {
    const todo = mk({ start: '2026-11-01', flex: 'month', flexDone: { '2026-11': '2026-11-17' } });
    const o = occurrences(todo, '2026-11-01', '2026-11-30')[0];
    expect([o.done, occDoneDay(o)]).toEqual([true, undefined]);
  });

  it('darkens the done part, piece by piece', () => {
    // 5 days, 60 %: three days are done
    const o = occ(cat, today);
    expect(barLabel(o, o.start, 5, today, all).fill).toBeCloseTo(0.6);
    // cut by the week's end after two days: the first piece is full, a third of the second
    expect(barLabel(o, o.start, 2, today, all).fill).toBe(1);
    expect(barLabel(o, addDays(o.start, 2), 3, today, all).fill).toBeCloseTo(1 / 3);
    // nothing darker without the progress, at 0 %, or once it is done (the whole bar is pale)
    expect(barLabel(o, o.start, 5, today, { ...all, progress: false }).fill).toBe(0);
    const fresh = mk({ start: today, end: addDays(today, 2) });
    expect(barLabel(occ(fresh), today, 3, today, all).fill).toBe(0);
    const done = mk({ start: today, end: addDays(today, 2), done: true, progress: 100 });
    expect(barLabel(occ(done), today, 3, today, all).fill).toBe(0);
  });
});
