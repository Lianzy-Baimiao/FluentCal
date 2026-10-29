import type { HolidayYear } from '@/types';
import { isISODate } from './date';

// China's statutory holiday schedule (incl. 调休 working days), scraped from gov.cn by the
// holiday-cn project. cdn.jsdelivr.net is unreachable on some networks, so try mirrors.
const SOURCES = [
  (y: number) => `https://fastly.jsdelivr.net/gh/NateScarlet/holiday-cn@master/${y}.json`,
  (y: number) => `https://raw.githubusercontent.com/NateScarlet/holiday-cn/master/${y}.json`,
];

/** Fetches one year; null when every source failed. An unpublished year returns days: []. */
export async function fetchHolidayYear(year: number, fetchImpl: typeof fetch = fetch): Promise<HolidayYear | null> {
  for (const src of SOURCES) {
    const ctl = new AbortController();
    const timer = setTimeout(() => ctl.abort(), 8000);
    try {
      const r = await fetchImpl(src(year), { signal: ctl.signal, cache: 'no-store' });
      if (!r.ok) continue;
      const j = (await r.json()) as { year?: unknown; days?: unknown };
      if (j.year !== year || !Array.isArray(j.days)) continue;
      const days = (j.days as { date?: unknown; name?: unknown; isOffDay?: unknown }[])
        .filter((d) => isISODate(d.date) && typeof d.name === 'string')
        .map((d) => ({ date: d.date as string, name: d.name as string, isOffDay: !!d.isOffDay }));
      return { year, fetchedAt: Date.now(), days };
    } catch {
      // network error or timeout: try the next source
    } finally {
      clearTimeout(timer);
    }
  }
  return null;
}
