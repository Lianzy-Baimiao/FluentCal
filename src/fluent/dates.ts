// Minimal local-date helpers for the date controls (ISO strings, YYYY-MM-DD).

export const pad = (n: number) => String(n).padStart(2, '0');

export function iso(y: number, m: number, d: number): string {
  return `${y}-${pad(m)}-${pad(d)}`;
}

export function fromDate(d: Date): string {
  return iso(d.getFullYear(), d.getMonth() + 1, d.getDate());
}

export function toDate(s: string): Date | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  if (!m) return null;
  const d = new Date(+m[1], +m[2] - 1, +m[3]);
  return d.getMonth() === +m[2] - 1 ? d : null;
}

export const todayIso = () => fromDate(new Date());

export function addDays(s: string, n: number): string {
  const d = toDate(s) ?? new Date();
  d.setDate(d.getDate() + n);
  return fromDate(d);
}

export const daysInMonth = (y: number, m: number) => new Date(y, m, 0).getDate();

/** Parses typed dates: 2026-10-30, 2026/10/30, 2026.10.30, 2026年10月30日, 10/30, 10-30,
 *  10月30日 (current year), 今天/明天/后天/昨天. Returns null when invalid. */
export function parseDateInput(text: string, base = new Date()): string | null {
  const t = text.trim();
  const rel: Record<string, number> = { 今天: 0, 今日: 0, 明天: 1, 后天: 2, 昨天: -1, 前天: -2 };
  if (t in rel) return addDays(fromDate(base), rel[t]);
  let m = /^(\d{4})\s*[-/.年]\s*(\d{1,2})\s*[-/.月]\s*(\d{1,2})\s*日?$/.exec(t);
  let y = base.getFullYear(), mo: number, d: number;
  if (m) {
    y = +m[1]; mo = +m[2]; d = +m[3];
  } else if ((m = /^(\d{1,2})\s*[-/.月]\s*(\d{1,2})\s*日?$/.exec(t))) {
    mo = +m[1]; d = +m[2];
  } else if ((m = /^(\d{4})(\d{2})(\d{2})$/.exec(t))) {
    y = +m[1]; mo = +m[2]; d = +m[3];
  } else return null;
  if (mo < 1 || mo > 12 || d < 1 || d > daysInMonth(y, mo)) return null;
  return iso(y, mo, d);
}

/** Parses typed times: 9, 930, 0930, 9:30, 21.05, 9点, 9点半, 下午3点. Returns HH:mm or null. */
export function parseTimeInput(text: string): string | null {
  let t = text.trim().replace(/：/g, ':');
  let pm = false;
  if (/^(下午|晚上|pm)/i.test(t)) { pm = true; t = t.replace(/^(下午|晚上|pm)\s*/i, ''); }
  else t = t.replace(/^(上午|早上|am)\s*/i, '');
  let h: number, mi = 0, m: RegExpExecArray | null;
  if ((m = /^(\d{1,2})\s*点\s*(半|(\d{1,2})\s*分?)?$/.exec(t))) {
    h = +m[1]; mi = m[2] === '半' ? 30 : m[3] ? +m[3] : 0;
  } else if ((m = /^(\d{1,2})[:.](\d{1,2})$/.exec(t))) {
    h = +m[1]; mi = +m[2];
  } else if ((m = /^(\d{1,4})$/.exec(t))) {
    const s = m[1];
    if (s.length <= 2) h = +s;
    else { h = +s.slice(0, s.length - 2); mi = +s.slice(-2); }
  } else return null;
  if (pm && h < 12) h += 12;
  if (h > 23 || mi > 59) return null;
  return `${pad(h)}:${pad(mi)}`;
}
