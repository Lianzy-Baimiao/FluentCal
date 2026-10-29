import type { ISODate } from '@/types';
import { addDays, diffDays } from './date';
import type { Occurrence } from './recur';

/** A task occurrence clipped to one week row. */
export interface Chip {
  key: string;
  occ: Occurrence;
  /** 0–6 */
  col: number;
  span: number;
  lane: number;
  /** Continues from the previous / into the next week. */
  contLeft: boolean;
  contRight: boolean;
  multiDay: boolean;
  visible: boolean;
}

export interface WeekLayout {
  chips: Chip[];
  /** Hidden chip count per column ("+N"). */
  overflow: number[];
  /** Lane index where the "+N" button sits, per column. */
  overflowLane: number[];
}

export interface LayoutOptions {
  /** Lanes that fit in a cell; Infinity = unlimited. */
  capacity: number;
  completedLast: boolean;
}

function sortKey(a: Occurrence, b: Occurrence, completedLast: boolean): number {
  const am = a.start !== a.end ? 0 : 1;
  const bm = b.start !== b.end ? 0 : 1;
  if (am !== bm) return am - bm; // multi-day bars first, so they get the top lanes
  if (a.start !== b.start) return a.start < b.start ? -1 : 1;
  const al = diffDays(a.start, a.end);
  const bl = diffDays(b.start, b.end);
  if (al !== bl) return bl - al;
  if (completedLast && a.done !== b.done) return a.done ? 1 : -1;
  const at = a.task.time ?? '';
  const bt = b.task.time ?? '';
  if (at !== bt) return at === '' ? -1 : bt === '' ? 1 : at < bt ? -1 : 1; // all-day first, then by time
  return a.task.order - b.task.order;
}

/**
 * Lays out occurrences in a week row starting at `weekStart`: assigns each chip the lowest
 * lane free across all its columns, then hides what doesn't fit. A column that overflows
 * gives its last visible lane to a "+N" button.
 */
export function layoutWeek(weekStart: ISODate, occs: Occurrence[], opt: LayoutOptions): WeekLayout {
  const weekEnd = addDays(weekStart, 6);
  const sorted = occs
    .filter((o) => o.end >= weekStart && o.start <= weekEnd)
    .sort((a, b) => sortKey(a, b, opt.completedLast));

  const used: boolean[][] = Array.from({ length: 7 }, () => []);
  const chips: Chip[] = [];
  for (const occ of sorted) {
    const s = occ.start < weekStart ? weekStart : occ.start;
    const e = occ.end > weekEnd ? weekEnd : occ.end;
    const col = diffDays(weekStart, s);
    const span = diffDays(s, e) + 1;
    let lane = 0;
    while (Array.from({ length: span }, (_, i) => used[col + i][lane]).some(Boolean)) lane++;
    for (let i = 0; i < span; i++) used[col + i][lane] = true;
    chips.push({
      key: `${occ.task.id}@${occ.date}`,
      occ,
      col,
      span,
      lane,
      contLeft: occ.start < weekStart,
      contRight: occ.end > weekEnd,
      multiDay: occ.start !== occ.end,
      visible: true,
    });
  }

  const cap = opt.capacity;
  const overflow = Array(7).fill(0);
  const overflowLane = Array(7).fill(0);
  if (!Number.isFinite(cap)) return { chips, overflow, overflowLane };

  // Lanes each column may show: all when they fit, else one fewer (room for "+N").
  const maxLane = Array(7).fill(-1);
  for (const c of chips) for (let i = c.col; i < c.col + c.span; i++) maxLane[i] = Math.max(maxLane[i], c.lane);
  const limit = maxLane.map((m) => (m + 1 <= cap ? cap : Math.max(0, cap - 1)));
  for (const c of chips) {
    for (let i = c.col; i < c.col + c.span; i++) {
      if (c.lane >= limit[i]) c.visible = false;
    }
  }
  for (const c of chips) {
    if (c.visible) continue;
    for (let i = c.col; i < c.col + c.span; i++) overflow[i]++;
  }
  for (let i = 0; i < 7; i++) {
    if (!overflow[i]) continue;
    // Put the button in the first lane of this column not used by a visible chip.
    const busy = new Set(chips.filter((c) => c.visible && c.col <= i && i < c.col + c.span).map((c) => c.lane));
    let lane = 0;
    while (busy.has(lane)) lane++;
    overflowLane[i] = Math.min(lane, Math.max(0, cap - 1));
  }
  return { chips, overflow, overflowLane };
}
