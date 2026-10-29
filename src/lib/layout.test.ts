import { describe, expect, it } from 'vitest';
import { layoutWeek } from './layout';
import { occurrences } from './recur';
import { mk } from './testing';

const week = '2026-10-26'; // Monday
const occs = (tasks: ReturnType<typeof mk>[]) => tasks.flatMap((t) => occurrences(t, week, '2026-11-01'));

describe('week layout', () => {
  const bar = mk({ id: 'bar', start: '2026-10-26', end: '2026-10-28' });
  const mon1 = mk({ id: 'mon1', start: '2026-10-26' });
  const mon2 = mk({ id: 'mon2', start: '2026-10-26' });
  const tue = mk({ id: 'tue', start: '2026-10-27' });

  it('puts multi-day bars in the top lanes', () => {
    const l = layoutWeek(week, occs([mon1, tue, bar, mon2]), { capacity: Infinity, completedLast: true });
    const by = Object.fromEntries(l.chips.map((c) => [c.occ.task.id, c]));
    expect(by.bar).toMatchObject({ col: 0, span: 3, lane: 0, multiDay: true });
    expect(by.mon1.lane).toBe(1);
    expect(by.mon2.lane).toBe(2);
    expect(by.tue).toMatchObject({ col: 1, lane: 1 });
  });

  it('hides what does not fit and reserves a lane for "+N"', () => {
    const l = layoutWeek(week, occs([mon1, tue, bar, mon2]), { capacity: 2, completedLast: true });
    const by = Object.fromEntries(l.chips.map((c) => [c.occ.task.id, c]));
    expect(by.bar.visible).toBe(true);
    expect(by.mon1.visible).toBe(false);
    expect(by.mon2.visible).toBe(false);
    expect(by.tue.visible).toBe(true);
    expect(l.overflow[0]).toBe(2);
    expect(l.overflowLane[0]).toBe(1);
    expect(l.overflow[1]).toBe(0);
  });

  it('clips bars that cross week boundaries', () => {
    const long = mk({ start: '2026-10-24', end: '2026-11-03' });
    const [c] = layoutWeek(week, occs([long]), { capacity: Infinity, completedLast: true }).chips;
    expect(c).toMatchObject({ col: 0, span: 7, contLeft: true, contRight: true });
  });

  it('sorts completed single-day tasks last', () => {
    const a = mk({ start: '2026-10-29', done: true });
    const b = mk({ start: '2026-10-29' });
    const l = layoutWeek(week, occs([a, b]), { capacity: Infinity, completedLast: true });
    expect(l.chips.map((c) => c.occ.task.id)).toEqual([b.id, a.id]);
  });
});
