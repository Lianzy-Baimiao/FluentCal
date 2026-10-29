import { LegalHoliday, LunarDay, SolarDay } from 'tyme4ts';
import type { HolidayYear, ISODate } from '@/types';
import { parts, weekday, ymd } from './date';

/** What a day cell shows under the date number, plus tooltip details. */
export interface DayInfo {
  /** Short label: festival, solar term, lunar month (on day 1) or lunar day. */
  text: string;
  kind: 'lunar' | 'month' | 'term' | 'festival';
  lunarYear: number;
  lunarMonth: number; // negative = leap month (tyme4ts convention)
  lunarDay: number;
  /** e.g. "九月二十" */
  lunarLabel: string;
  /** e.g. "丙午年" */
  ganzhiYear: string;
  festivals: string[];
  term?: string;
  /** Statutory holiday: off day or adjusted working day (调休). */
  legal?: { name: string; work: boolean };
  recommends?: string[];
  avoids?: string[];
}

const FESTIVAL_RENAME: Record<string, string> = { 龙头节: '龙抬头', 冬至节: '冬至', 清明节: '清明' };
const MONTH_RENAME: Record<string, string> = { 十一月: '冬月', 十二月: '腊月' };
/** Fixed-date observances tyme4ts doesn't list. */
const EXTRA_FIXED: Record<string, string> = {
  '02-14': '情人节',
  '04-01': '愚人节',
  '12-24': '平安夜',
  '12-25': '圣诞节',
};

/** Second Sunday of May / third Sunday of June. */
function extraMovable(y: number, m: number, d: number): string | undefined {
  if (m !== 5 && m !== 6) return;
  const first = weekday(ymd(y, m, 1));
  const firstSunday = 1 + ((7 - first) % 7);
  if (m === 5 && d === firstSunday + 7) return '母亲节';
  if (m === 6 && d === firstSunday + 14) return '父亲节';
}

let overrides: Record<string, HolidayYear> = {};
/** Every date listed by any online year file (a year's file can list days of the previous
 * year, e.g. 2019.json has the 2018-12-29 调休 day). */
let overrideDays = new Map<ISODate, { name: string; isOffDay: boolean }>();
const cache = new Map<ISODate, DayInfo>();

/** Online holiday data (holiday-cn) replaces the built-in schedule for the years it covers. */
export function setHolidayOverrides(h: Record<string, HolidayYear>) {
  overrides = h;
  overrideDays = new Map();
  for (const y of Object.values(h)) for (const d of y.days) overrideDays.set(d.date, d);
  cache.clear();
  legalCache.clear();
}

const legalCache = new Map<ISODate, DayInfo['legal'] | null>();

/** Online data is authoritative for a date when its year was fetched; days near New Year
 * also need the neighbouring year's file, which may list them. */
function onlineCovers(y: number, m: number, d: number): boolean {
  const has = (yy: number) => (overrides[String(yy)]?.days.length ?? 0) > 0;
  if (!has(y)) return false;
  // A year's file may also list the last days of the previous year, so late December is only
  // covered once next year's file is in; otherwise the built-in data stays in charge.
  return !(m === 12 && d >= 20) || has(y + 1);
}

export function legalHoliday(iso: ISODate): DayInfo['legal'] | undefined {
  const hit = legalCache.get(iso);
  if (hit !== undefined) return hit ?? undefined;
  const [y, m, d] = parts(iso);
  let res: DayInfo['legal'] | null = null;
  const listed = overrideDays.get(iso);
  if (listed) {
    res = { name: listed.name, work: !listed.isOffDay };
  } else if (onlineCovers(y, m, d)) {
    res = null; // an ordinary day per the official schedule
  } else {
    try {
      const lh = LegalHoliday.fromYmd(y, m, d);
      if (lh) res = { name: lh.getName(), work: lh.isWork() };
    } catch {
      res = null;
    }
  }
  legalCache.set(iso, res);
  return res ?? undefined;
}

/** True when the day is a rest day: statutory holiday, or a configured weekend day not moved to work. */
export function isRestDay(iso: ISODate, restDays: number[] = [0, 6]): boolean {
  const lh = legalHoliday(iso);
  if (lh) return !lh.work;
  return restDays.includes(weekday(iso));
}

/** Working day per the official schedule (weekends + holidays off, 调休 days on). */
export function isLegalWorkday(iso: ISODate): boolean {
  return !isRestDay(iso, [0, 6]);
}

export function dayInfo(iso: ISODate): DayInfo {
  const hit = cache.get(iso);
  if (hit) return hit;
  const [y, m, d] = parts(iso);
  const sd = SolarDay.fromYmd(y, m, d);
  const ld = sd.getLunarDay();
  const lm = ld.getLunarMonth();
  const monthName = MONTH_RENAME[lm.getName()] ?? lm.getName();

  const festivals: string[] = [];
  const lf = ld.getFestival();
  if (lf) festivals.push(FESTIVAL_RENAME[lf.getName()] ?? lf.getName());
  const sf = sd.getFestival();
  if (sf) festivals.push(sf.getName());
  const extra = EXTRA_FIXED[iso.slice(5)] ?? extraMovable(y, m, d);
  if (extra) festivals.push(extra);

  const td = sd.getTermDay();
  const term = td.getDayIndex() === 0 ? td.getSolarTerm().getName() : undefined;

  let text: string;
  let kind: DayInfo['kind'];
  const lunarDay = ld.getDay();
  // A festival on the same day as its solar term (清明, 冬至) shows once.
  const shownFestival = festivals.find((f) => f !== term);
  if (shownFestival) {
    text = shownFestival;
    kind = 'festival';
  } else if (term) {
    text = term;
    kind = 'term';
  } else if (lunarDay === 1) {
    text = monthName;
    kind = 'month';
  } else {
    text = ld.getName();
    kind = 'lunar';
  }

  const info: DayInfo = {
    text,
    kind,
    lunarYear: lm.getYear(),
    lunarMonth: lm.getMonthWithLeap(),
    lunarDay,
    lunarLabel: `${monthName}${ld.getName()}`,
    ganzhiYear: `${ld.getYearSixtyCycle().getName()}年`,
    festivals: festivals.filter((f, i) => festivals.indexOf(f) === i),
    term,
    legal: legalHoliday(iso),
  };
  cache.set(iso, info);
  return info;
}

/** 宜/忌 for the day (computed lazily; only shown on hover). */
export function almanac(iso: ISODate): { recommends: string[]; avoids: string[] } {
  const [y, m, d] = parts(iso);
  const ld = SolarDay.fromYmd(y, m, d).getLunarDay();
  return {
    recommends: ld.getRecommends().map((t) => t.getName()),
    avoids: ld.getAvoids().map((t) => t.getName()),
  };
}

/**
 * Solar date of a lunar date in a given lunar year. A leap month falls back to the regular
 * month when that year has no such leap month; day 30 falls back to 29 in short months.
 */
export function lunarToSolar(lunarYear: number, lunarMonth: number, lunarDay: number): ISODate | undefined {
  const tries: [number, number][] = [[lunarMonth, lunarDay]];
  if (lunarMonth < 0) tries.push([-lunarMonth, lunarDay]);
  for (const [mm, dd] of [...tries, ...tries.map(([mm]) => [mm, Math.min(lunarDay, 29)] as [number, number])]) {
    try {
      const s = LunarDay.fromYmd(lunarYear, mm, dd).getSolarDay();
      return ymd(s.getYear(), s.getMonth(), s.getDay());
    } catch {
      // invalid in this year, try the next fallback
    }
  }
  return undefined;
}

/** Chinese numerals for lunar months, used in rule descriptions. */
export function lunarMonthName(m: number): string {
  const names = ['正', '二', '三', '四', '五', '六', '七', '八', '九', '十', '冬', '腊'];
  return `${m < 0 ? '闰' : ''}${names[Math.abs(m) - 1]}月`;
}

export function lunarDayName(d: number): string {
  const ten = ['初', '十', '廿', '三'];
  const digits = ['', '一', '二', '三', '四', '五', '六', '七', '八', '九', '十'];
  if (d === 10) return '初十';
  if (d === 20) return '二十';
  if (d === 30) return '三十';
  return ten[Math.floor(d / 10)] + digits[d % 10];
}
