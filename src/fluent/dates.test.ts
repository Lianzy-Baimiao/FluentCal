import { describe, expect, it } from 'vitest';
import { addDays, parseDateInput, parseTimeInput, toDate } from './dates';

const base = new Date(2026, 9, 29); // 2026-10-29

describe('parseDateInput', () => {
  it('accepts common formats', () => {
    for (const s of ['2026-10-30', '2026/10/30', '2026.10.30', '2026年10月30日', '20261030', '10/30', '10-30', '10月30日']) {
      expect(parseDateInput(s, base), s).toBe('2026-10-30');
    }
  });
  it('understands relative words', () => {
    expect(parseDateInput('今天', base)).toBe('2026-10-29');
    expect(parseDateInput('明天', base)).toBe('2026-10-30');
    expect(parseDateInput('昨天', base)).toBe('2026-10-28');
  });
  it('rejects invalid dates', () => {
    for (const s of ['2026-02-30', '13/01', 'abc', '', '2026-1-']) expect(parseDateInput(s, base), s).toBeNull();
  });
});

describe('parseTimeInput', () => {
  it('accepts common formats', () => {
    const cases: Record<string, string> = {
      '9': '09:00', '930': '09:30', '0930': '09:30', '9:30': '09:30', '21.05': '21:05', '21：05': '21:05',
      '9点': '09:00', '9点半': '09:30', '9点15分': '09:15', '下午3点': '15:00', 'pm 3:30': '15:30', '上午9点': '09:00',
    };
    for (const [s, v] of Object.entries(cases)) expect(parseTimeInput(s), s).toBe(v);
  });
  it('rejects invalid times', () => {
    for (const s of ['24:00', '9:60', 'x', '']) expect(parseTimeInput(s), s).toBeNull();
  });
});

describe('date helpers', () => {
  it('adds days across months and validates', () => {
    expect(addDays('2026-10-31', 1)).toBe('2026-11-01');
    expect(addDays('2026-03-01', -1)).toBe('2026-02-28');
    expect(toDate('2026-02-29')).toBeNull();
  });
});
