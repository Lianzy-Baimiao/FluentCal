import { afterEach, describe, expect, it } from 'vitest';
import { dayInfo, isLegalWorkday, isRestDay, legalHoliday, lunarDayName, lunarMonthName, lunarToSolar, setHolidayOverrides } from './lunar';

describe('lunar calendar', () => {
  afterEach(() => setHolidayOverrides({}));

  it('labels festivals, solar terms and lunar days', () => {
    expect(dayInfo('2026-02-17').text).toBe('春节');
    expect(dayInfo('2026-04-05')).toMatchObject({ text: '清明', kind: 'term' });
    const d = dayInfo('2026-10-29');
    expect(d.lunarLabel).toBe('九月二十');
    expect(d.text).toBe('二十');
    expect(d.ganzhiYear).toBe('丙午年');
    expect(dayInfo('2026-10-01').festivals).toContain('国庆节');
    expect(dayInfo('2026-05-10').festivals).toContain('母亲节');
    expect(dayInfo('2026-06-21').festivals).toContain('父亲节');
  });

  it('marks statutory holidays and adjusted working days', () => {
    expect(dayInfo('2026-10-01').legal).toEqual({ name: '国庆节', work: false });
    expect(dayInfo('2026-10-10').legal).toEqual({ name: '国庆节', work: true });
    expect(isLegalWorkday('2026-10-10')).toBe(true); // Saturday, 调休
    expect(isLegalWorkday('2026-10-01')).toBe(false);
    expect(isLegalWorkday('2026-10-12')).toBe(true);
    expect(isRestDay('2026-10-31')).toBe(true); // ordinary Saturday
  });

  it('lets online data replace a whole year', () => {
    setHolidayOverrides({ '2026': { year: 2026, fetchedAt: 0, days: [{ date: '2026-03-02', name: '测试假', isOffDay: true }] } });
    expect(isRestDay('2026-03-02')).toBe(true);
    expect(legalHoliday('2026-10-01')).toBeUndefined();
    expect(legalHoliday('2025-10-01')?.name).toBe('国庆中秋'); // other years keep the built-in data
  });

  it('uses dates listed in the next year file and keeps built-in data at year end otherwise', () => {
    const y2018 = { year: 2018, fetchedAt: 0, days: [{ date: '2018-02-16', name: '春节', isOffDay: true }] };
    // holiday-cn's 2019.json lists the 2018 year-end days
    const y2019 = { year: 2019, fetchedAt: 0, days: [{ date: '2018-12-29', name: '元旦', isOffDay: false }, { date: '2018-12-31', name: '元旦', isOffDay: true }, { date: '2019-01-01', name: '元旦', isOffDay: true }] };
    setHolidayOverrides({ '2018': y2018, '2019': y2019 });
    expect(legalHoliday('2018-12-29')).toEqual({ name: '元旦', work: true });
    expect(legalHoliday('2018-12-31')).toEqual({ name: '元旦', work: false });
    expect(legalHoliday('2018-06-01')).toBeUndefined(); // ordinary day per the 2018 file
    setHolidayOverrides({ '2018': y2018 }); // 2019 not fetched: year end falls back to built-in data
    expect(legalHoliday('2018-12-31')?.work).toBe(false);
  });

  it('converts lunar dates to solar dates', () => {
    expect(lunarToSolar(2027, 1, 1)).toBe('2027-02-06');
    expect(lunarToSolar(2025, -6, 1)).toBe('2025-07-25');
    expect(lunarToSolar(2026, -6, 1)).toBe(lunarToSolar(2026, 6, 1)); // no leap month that year
  });

  it('names lunar months and days', () => {
    expect(lunarMonthName(1)).toBe('正月');
    expect(lunarMonthName(-6)).toBe('闰六月');
    expect(lunarMonthName(12)).toBe('腊月');
    expect(lunarDayName(1)).toBe('初一');
    expect(lunarDayName(15)).toBe('十五');
    expect(lunarDayName(21)).toBe('廿一');
    expect(lunarDayName(30)).toBe('三十');
  });
});
