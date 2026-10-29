import { describe, expect, it } from 'vitest';
import {
  addDays,
  addMonths,
  dayNum,
  diffDays,
  fromDayNum,
  isISODate,
  isoWeek,
  monthWeeks,
  relativeDay,
  startOfWeek,
  weekday,
} from './date';

describe('date', () => {
  it('converts day numbers both ways', () => {
    for (const d of ['1970-01-01', '2000-02-29', '2026-10-29', '2099-12-31']) {
      expect(fromDayNum(dayNum(d))).toBe(d);
    }
    expect(dayNum('1970-01-02')).toBe(1);
  });

  it('adds days across month and year ends', () => {
    expect(addDays('2026-12-31', 1)).toBe('2027-01-01');
    expect(addDays('2024-03-01', -1)).toBe('2024-02-29');
    expect(diffDays('2026-10-01', '2026-11-01')).toBe(31);
  });

  it('knows weekdays', () => {
    expect(weekday('2026-10-29')).toBe(4); // Thursday
    expect(weekday('1970-01-01')).toBe(4);
    expect(startOfWeek('2026-10-29', 1)).toBe('2026-10-26');
    expect(startOfWeek('2026-10-29', 0)).toBe('2026-10-25');
    expect(startOfWeek('2026-10-25', 1)).toBe('2026-10-19'); // Sunday belongs to the previous Monday week
  });

  it('adds months with day clamping', () => {
    expect(addMonths('2026-01-31', 1)).toBe('2026-02-28');
    expect(addMonths('2024-01-31', 1)).toBe('2024-02-29');
    expect(addMonths('2026-03-15', -3)).toBe('2025-12-15');
    expect(addMonths('2026-11-30', 14)).toBe('2028-01-30');
  });

  it('computes ISO week numbers', () => {
    expect(isoWeek('2026-01-01')).toBe(1);
    expect(isoWeek('2026-12-31')).toBe(53);
    expect(isoWeek('2027-01-01')).toBe(53);
    expect(isoWeek('2027-01-04')).toBe(1);
    expect(isoWeek('2021-01-03')).toBe(53);
  });

  it('builds month week rows', () => {
    const auto = monthWeeks(2026, 10, 1, 'auto');
    expect(auto[0]).toBe('2026-09-28');
    expect(auto).toHaveLength(5);
    expect(monthWeeks(2026, 10, 1, 'six')).toHaveLength(6);
    expect(monthWeeks(2026, 2, 0, 'auto')).toHaveLength(4); // Feb 2026 starts on Sunday, 28 days
  });

  it('validates ISO dates', () => {
    expect(isISODate('2026-02-29')).toBe(false);
    expect(isISODate('2024-02-29')).toBe(true);
    expect(isISODate('2026-13-01')).toBe(false);
    expect(isISODate('20261001')).toBe(false);
  });

  it('describes relative days', () => {
    expect(relativeDay('2026-10-30', '2026-10-29')).toBe('明天');
    expect(relativeDay('2026-10-29', '2026-10-29')).toBe('今天');
    expect(relativeDay('2026-11-05', '2026-10-29')).toBe('7 天后');
  });
});
