import { describe, expect, it } from 'vitest';
import { toICS } from './export';
import { mk } from './testing';

const NOW = '2026-10-29';
const ics = (...t: ReturnType<typeof mk>[]) => toICS(t, {}, NOW);
const rule = (s: string) => s.match(/RRULE:(.*)/)?.[1];
const dtstart = (s: string) => s.match(/DTSTART[^:]*:(\d{8})/)?.[1];

describe('ICS repeat rules match the app', () => {
  it('counts "every 2 weeks" from Sunday like the app', () => {
    const r = rule(ics(mk({ start: '2026-10-04', repeat: { freq: 'weekly', interval: 2, weekdays: [0, 1] } })));
    expect(r).toBe('FREQ=WEEKLY;INTERVAL=2;WKST=SU;BYDAY=SU,MO');
  });

  it('exports the last weekday of a month as -1', () => {
    // 2026-02-22 is the 4th Sunday and also the last one of February.
    expect(rule(ics(mk({ start: '2026-02-22', repeat: { freq: 'monthly', monthMode: 'nthWeekday' } })))).toContain('BYDAY=-1SU');
    expect(rule(ics(mk({ start: '2026-03-22', repeat: { freq: 'monthly', monthMode: 'nthWeekday' } })))).toContain('BYDAY=4SU');
  });

  it('clamps the 31st to the month end', () => {
    expect(rule(ics(mk({ start: '2026-01-31', repeat: { freq: 'monthly' } })))).toBe('FREQ=MONTHLY;INTERVAL=1;BYMONTHDAY=28,29,30,31;BYSETPOS=-1');
    expect(rule(ics(mk({ start: '2024-02-29', repeat: { freq: 'yearly' } })))).toContain('BYMONTHDAY=28,29;BYSETPOS=-1');
  });

  it('starts at the first real occurrence', () => {
    expect(dtstart(ics(mk({ start: '2026-10-15', repeat: { freq: 'monthLastDay' } })))).toBe('20261031');
    expect(dtstart(ics(mk({ start: '2026-10-31', repeat: { freq: 'workday' } })))).toBe('20261102'); // Saturday → Monday
  });

  it('writes UNTIL with the same type as DTSTART', () => {
    expect(rule(ics(mk({ start: '2026-10-01', time: '09:30', repeat: { freq: 'daily', until: '2026-10-10' } })))).toContain('UNTIL=20261010T235959');
    expect(rule(ics(mk({ start: '2026-10-01', repeat: { freq: 'daily', until: '2026-10-10' } })))).toMatch(/UNTIL=20261010$/);
  });

  it('exports current dates of old lunar repeats', () => {
    const out = ics(mk({ title: '生日', start: '1990-04-10', repeat: { freq: 'lunarYearly' } }));
    const years = [...out.matchAll(/DTSTART;VALUE=DATE:(\d{4})/g)].map((m) => m[1]);
    expect(years).toContain('2026');
    expect(years).toContain('2027');
    expect(years).not.toContain('1990');
  });
});
