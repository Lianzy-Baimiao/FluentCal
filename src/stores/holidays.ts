import { doc, patchSettings, put } from './doc';
import { fetchHolidayYear } from '@/lib/holidayFetch';
import type { HolidayYear } from '@/types';

const INTERVAL = 3 * 86_400_000;

/**
 * Refreshes the statutory holiday schedule for this and next year. Runs at most every three
 * days unless forced. A year that is not published yet (empty list) keeps the built-in data.
 */
export async function updateHolidays(force = false): Promise<{ updated: number[]; failed: boolean }> {
  const h = doc.settings.holidays;
  if (!force && (!h.autoUpdate || Date.now() - h.lastCheck < INTERVAL)) return { updated: [], failed: false };
  const y = new Date().getFullYear();
  const res = await Promise.all([y, y + 1].map((yy) => fetchHolidayYear(yy)));
  const items: Record<string, HolidayYear> = {};
  for (const r of res) if (r && r.days.length) items[String(r.year)] = r;
  if (Object.keys(items).length) await put('holidays', items);
  if (res.some(Boolean)) patchSettings({ holidays: { lastCheck: Date.now() } });
  return { updated: Object.keys(items).map(Number), failed: res.every((r) => r === null) };
}
