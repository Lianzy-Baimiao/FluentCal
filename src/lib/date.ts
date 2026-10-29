import type { ISODate } from '@/types';

// Dates are handled as day numbers (days since 1970-01-01, computed in UTC) so adding days
// never drifts across DST changes and range math stays integer.

const MS_DAY = 86_400_000;

export const pad2 = (n: number) => String(n).padStart(2, '0');

export function ymd(y: number, m: number, d: number): ISODate {
  return `${y}-${pad2(m)}-${pad2(d)}`;
}

export function parts(iso: ISODate): [number, number, number] {
  return [+iso.slice(0, 4), +iso.slice(5, 7), +iso.slice(8, 10)];
}

export function isISODate(s: unknown): s is ISODate {
  if (typeof s !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(s)) return false;
  const [y, m, d] = parts(s);
  return m >= 1 && m <= 12 && d >= 1 && d <= daysInMonth(y, m);
}

export function dayNum(iso: ISODate): number {
  const [y, m, d] = parts(iso);
  return Math.floor(Date.UTC(y, m - 1, d) / MS_DAY);
}

export function fromDayNum(n: number): ISODate {
  const dt = new Date(n * MS_DAY);
  return ymd(dt.getUTCFullYear(), dt.getUTCMonth() + 1, dt.getUTCDate());
}

export function addDays(iso: ISODate, n: number): ISODate {
  return fromDayNum(dayNum(iso) + n);
}

/** b - a in days */
export function diffDays(a: ISODate, b: ISODate): number {
  return dayNum(b) - dayNum(a);
}

export function toISO(d: Date): ISODate {
  return ymd(d.getFullYear(), d.getMonth() + 1, d.getDate());
}

export function today(): ISODate {
  return toISO(new Date());
}

/** 0 = Sunday … 6 = Saturday */
export function weekday(iso: ISODate): number {
  return (((dayNum(iso) + 4) % 7) + 7) % 7; // 1970-01-01 was a Thursday
}

export function daysInMonth(y: number, m: number): number {
  return new Date(Date.UTC(y, m, 0)).getUTCDate();
}

export function isLeapYear(y: number): boolean {
  return (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0;
}

export function startOfWeek(iso: ISODate, weekStart: 0 | 1): ISODate {
  return addDays(iso, -((weekday(iso) - weekStart + 7) % 7));
}

export function startOfMonth(iso: ISODate): ISODate {
  return iso.slice(0, 8) + '01';
}

export function endOfMonth(iso: ISODate): ISODate {
  const [y, m] = parts(iso);
  return ymd(y, m, daysInMonth(y, m));
}

/** Adds months, clamping the day (Jan 31 + 1 month = Feb 28/29). */
export function addMonths(iso: ISODate, n: number): ISODate {
  const [y, m, d] = parts(iso);
  const total = y * 12 + (m - 1) + n;
  const ny = Math.floor(total / 12);
  const nm = (total % 12) + 1;
  return ymd(ny, nm, Math.min(d, daysInMonth(ny, nm)));
}

export function minDate(a: ISODate, b: ISODate): ISODate {
  return a < b ? a : b;
}

export function maxDate(a: ISODate, b: ISODate): ISODate {
  return a > b ? a : b;
}

/** ISO 8601 week number. */
export function isoWeek(iso: ISODate): number {
  // The week belongs to the year containing its Thursday.
  const thursday = dayNum(iso) - ((weekday(iso) + 6) % 7) + 3;
  const year = new Date(thursday * MS_DAY).getUTCFullYear();
  return Math.floor((thursday - dayNum(ymd(year, 1, 1))) / 7) + 1;
}

/**
 * Weeks shown for a month: from the week containing the 1st through the week containing the
 * last day ('auto', 4–6 rows) or always 6 rows.
 */
export function monthWeeks(year: number, month: number, weekStart: 0 | 1, rows: 'auto' | 'six'): ISODate[] {
  const first = ymd(year, month, 1);
  const last = ymd(year, month, daysInMonth(year, month));
  const start = startOfWeek(first, weekStart);
  const count = rows === 'six' ? 6 : Math.floor(diffDays(start, last) / 7) + 1;
  return Array.from({ length: count }, (_, i) => addDays(start, i * 7));
}

export const WEEKDAY_SHORT = ['日', '一', '二', '三', '四', '五', '六'];

export function formatMD(iso: ISODate): string {
  const [, m, d] = parts(iso);
  return `${m}月${d}日`;
}

export function formatYMD(iso: ISODate): string {
  const [y, m, d] = parts(iso);
  return `${y}年${m}月${d}日`;
}

/** "10月30日 周五" or with the year when it differs from the current one. */
export function formatFriendly(iso: ISODate, now: ISODate = today()): string {
  const [y] = parts(iso);
  const base = y === parts(now)[0] ? formatMD(iso) : formatYMD(iso);
  return `${base} 周${WEEKDAY_SHORT[weekday(iso)]}`;
}

export function formatRange(start: ISODate, end: ISODate): string {
  if (start === end) return formatFriendly(start);
  return `${formatFriendly(start)} – ${formatFriendly(end)}`;
}

export function relativeDay(iso: ISODate, now: ISODate = today()): string {
  const d = diffDays(now, iso);
  if (d === 0) return '今天';
  if (d === 1) return '明天';
  if (d === 2) return '后天';
  if (d === -1) return '昨天';
  if (d === -2) return '前天';
  return d > 0 ? `${d} 天后` : `${-d} 天前`;
}
