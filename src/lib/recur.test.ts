import { describe, expect, it } from 'vitest';
import { describeRule, nextOccurrence, occurrences } from './recur';
import { mk } from './testing';

const dates = (...a: Parameters<typeof occurrences>) => occurrences(...a).map((o) => o.date);

describe('recurrence', () => {
  it('returns a plain task when it overlaps the range', () => {
    const t = mk({ start: '2026-10-28', end: '2026-11-02' });
    expect(dates(t, '2026-11-01', '2026-11-30')).toEqual(['2026-10-28']);
    expect(dates(t, '2026-11-03', '2026-11-30')).toEqual([]);
  });

  it('repeats daily with an interval', () => {
    const t = mk({ start: '2026-10-01', repeat: { freq: 'daily', interval: 2 } });
    expect(dates(t, '2026-10-01', '2026-10-10')).toEqual(['2026-10-01', '2026-10-03', '2026-10-05', '2026-10-07', '2026-10-09']);
  });

  it('repeats weekly on chosen weekdays', () => {
    const t = mk({ start: '2026-10-28', repeat: { freq: 'weekly', weekdays: [1, 3] } });
    expect(dates(t, '2026-10-26', '2026-11-08')).toEqual(['2026-10-28', '2026-11-02', '2026-11-04']);
    const biweekly = mk({ start: '2026-10-30', repeat: { freq: 'weekly', interval: 2, weekdays: [5] } });
    expect(dates(biweekly, '2026-10-01', '2026-12-01')).toEqual(['2026-10-30', '2026-11-13', '2026-11-27']);
  });

  it('clamps monthly dates to the month end', () => {
    const t = mk({ start: '2026-01-31', repeat: { freq: 'monthly' } });
    expect(dates(t, '2026-02-01', '2026-04-30')).toEqual(['2026-02-28', '2026-03-31', '2026-04-30']);
  });

  it('repeats on the Nth or last weekday of the month', () => {
    const second = mk({ start: '2026-10-13', repeat: { freq: 'monthly', monthMode: 'nthWeekday' } });
    expect(dates(second, '2026-11-01', '2026-12-31')).toEqual(['2026-11-10', '2026-12-08']);
    const last = mk({ start: '2026-10-27', repeat: { freq: 'monthly', monthMode: 'nthWeekday' } });
    expect(dates(last, '2026-11-01', '2026-12-31')).toEqual(['2026-11-24', '2026-12-29']);
  });

  it('handles Feb 29 yearly', () => {
    const t = mk({ start: '2024-02-29', repeat: { freq: 'yearly' } });
    expect(dates(t, '2025-01-01', '2028-12-31')).toEqual(['2025-02-28', '2026-02-28', '2027-02-28', '2028-02-29']);
  });

  it('stops after count or until', () => {
    expect(dates(mk({ start: '2026-10-01', repeat: { freq: 'daily', count: 3 } }), '2026-10-01', '2026-12-31')).toHaveLength(3);
    expect(dates(mk({ start: '2026-10-01', repeat: { freq: 'daily', until: '2026-10-04' } }), '2026-10-01', '2026-12-31')).toHaveLength(4);
    // count includes occurrences before the range
    expect(dates(mk({ start: '2026-10-01', repeat: { freq: 'daily', count: 3 } }), '2026-10-03', '2026-10-10')).toEqual(['2026-10-03']);
  });

  it('applies excluded and completed dates', () => {
    const t = mk({ start: '2026-10-01', repeat: { freq: 'daily' }, exDates: ['2026-10-02'], doneDates: ['2026-10-03'] });
    const occ = occurrences(t, '2026-10-01', '2026-10-04');
    expect(occ.map((o) => o.date)).toEqual(['2026-10-01', '2026-10-03', '2026-10-04']);
    expect(occ.map((o) => o.done)).toEqual([false, true, false]);
  });

  it('repeats on workdays and official working days', () => {
    expect(dates(mk({ start: '2026-10-30', repeat: { freq: 'workday' } }), '2026-10-30', '2026-11-03')).toEqual([
      '2026-10-30',
      '2026-11-02',
      '2026-11-03',
    ]);
    const legal = dates(mk({ start: '2026-09-28', repeat: { freq: 'legalWorkday' } }), '2026-10-01', '2026-10-12');
    expect(legal).toContain('2026-10-10');
    expect(legal).toContain('2026-10-12');
    expect(legal).not.toContain('2026-10-01');
    expect(legal).not.toContain('2026-10-03');
  });

  it('repeats on lunar dates and the month end', () => {
    expect(dates(mk({ start: '2026-02-17', repeat: { freq: 'lunarYearly' } }), '2027-01-01', '2027-12-31')).toEqual(['2027-02-06']);
    expect(dates(mk({ start: '2026-10-31', repeat: { freq: 'monthLastDay' } }), '2026-11-01', '2027-02-28')).toEqual([
      '2026-11-30',
      '2026-12-31',
      '2027-01-31',
      '2027-02-28',
    ]);
  });

  it('follows the Ebbinghaus schedule', () => {
    expect(dates(mk({ start: '2026-10-01', repeat: { freq: 'ebbinghaus' } }), '2026-09-01', '2027-01-01')).toEqual([
      '2026-10-01',
      '2026-10-02',
      '2026-10-03',
      '2026-10-05',
      '2026-10-08',
      '2026-10-16',
      '2026-10-31',
    ]);
  });

  it('includes multi-day occurrences that started before the range', () => {
    const t = mk({ start: '2026-10-30', end: '2026-10-31', repeat: { freq: 'weekly' } });
    expect(occurrences(t, '2026-10-31', '2026-10-31')).toMatchObject([{ date: '2026-10-30', end: '2026-10-31' }]);
  });

  it('stays fast for old unbounded rules', () => {
    const started = performance.now();
    expect(dates(mk({ start: '2000-01-01', repeat: { freq: 'daily' } }), '2026-10-01', '2026-10-07')).toHaveLength(7);
    expect(dates(mk({ start: '2000-01-03', repeat: { freq: 'weekly' } }), '2026-10-01', '2026-10-31')).toHaveLength(4);
    expect(performance.now() - started).toBeLessThan(50);
  });

  it('finds the next occurrence and describes rules', () => {
    const t = mk({ start: '2026-10-30', repeat: { freq: 'weekly', weekdays: [5] } });
    expect(nextOccurrence(t, '2026-10-31')).toBe('2026-11-06');
    expect(describeRule(t.repeat, t.start)).toBe('每周的周五');
    expect(describeRule({ freq: 'weekly', interval: 2, weekdays: [3, 1], count: 5 }, '2026-10-28')).toBe('每 2 周的周一、周三，共 5 次');
    expect(describeRule({ freq: 'lunarYearly' }, '2026-02-17')).toBe('每年农历正月初一');
    expect(describeRule(undefined, '2026-10-01')).toBe('不重复');
  });
});
