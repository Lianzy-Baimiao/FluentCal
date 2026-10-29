import type { GanttScale, Group, ISODate, Task } from '@/types';
import { addDays, addMonths, dayNum, diffDays, endOfMonth, isoWeek, maxDate, minDate, parts, startOfMonth, startOfWeek, weekday } from './date';

/** Pixels per day for each zoom level. */
export const DAY_WIDTH: Record<GanttScale, number> = { day: 32, week: 12, month: 4 };
export const ROW_HEIGHT = 32;
export const UNGROUPED = '__ungrouped__';

export type GanttRow =
  | { kind: 'group'; key: string; group: Group; tasks: Task[]; start?: ISODate; end?: ISODate; progress: number }
  | { kind: 'task'; key: string; task: Task; groupId: string }
  | { kind: 'add'; key: string; groupId: string };

export interface RowOptions {
  hideDone: boolean;
  hideSingleDay: boolean;
  groupFilter: string[];
}

export const UNGROUPED_GROUP: Group = { id: UNGROUPED, name: '未分组', color: 'gray', order: 1e9 };

/** True when a task belongs in the Gantt chart under the given options. */
export function inGantt(t: Task, o: RowOptions): boolean {
  if (o.hideDone && t.done && !t.repeat) return false;
  if (o.hideSingleDay && !t.groupId && t.start === t.end && !t.milestone && !t.deps?.length) return false;
  return true;
}

/** Group rows (with summary), their task rows and an "add task" row each. */
export function buildRows(tasks: Task[], groups: Group[], o: RowOptions): GanttRow[] {
  const byGroup = new Map<string, Task[]>();
  const known = new Set(groups.map((g) => g.id));
  for (const t of tasks) {
    if (!inGantt(t, o)) continue;
    const gid = t.groupId && known.has(t.groupId) ? t.groupId : UNGROUPED;
    if (!byGroup.has(gid)) byGroup.set(gid, []);
    byGroup.get(gid)!.push(t);
  }
  const ordered = [...groups].sort((a, b) => a.order - b.order);
  if (byGroup.has(UNGROUPED)) ordered.push(UNGROUPED_GROUP);
  const rows: GanttRow[] = [];
  for (const g of ordered) {
    if (o.groupFilter.length && !o.groupFilter.includes(g.id)) continue;
    const list = (byGroup.get(g.id) ?? []).sort(
      (a, b) => (a.start < b.start ? -1 : a.start > b.start ? 1 : a.order - b.order),
    );
    const sum = summarize(list);
    rows.push({ kind: 'group', key: `g:${g.id}`, group: g, tasks: list, ...sum });
    if (g.collapsed) continue;
    for (const t of list) rows.push({ kind: 'task', key: `t:${t.id}`, task: t, groupId: g.id });
    rows.push({ kind: 'add', key: `a:${g.id}`, groupId: g.id });
  }
  return rows;
}

/** Time span and duration-weighted progress of a set of tasks. */
export function summarize(list: Task[]): { start?: ISODate; end?: ISODate; progress: number } {
  if (!list.length) return { progress: 0 };
  let start = list[0].start;
  let end = list[0].end;
  let weight = 0;
  let acc = 0;
  for (const t of list) {
    start = minDate(start, t.start);
    end = maxDate(end, t.end);
    const w = diffDays(t.start, t.end) + 1;
    weight += w;
    acc += w * (t.done ? 100 : t.progress);
  }
  return { start, end, progress: weight ? Math.round(acc / weight) : 0 };
}

/** Visible date range: covers all tasks and today with margins, aligned to whole months. */
export function chartRange(tasks: Task[], now: ISODate, scale: GanttScale): { start: ISODate; end: ISODate } {
  const pad = scale === 'day' ? 2 : scale === 'week' ? 4 : 12;
  let s = addMonths(now, -1);
  let e = addMonths(now, pad);
  for (const t of tasks) {
    s = minDate(s, t.start);
    e = maxDate(e, t.end);
  }
  return { start: startOfMonth(addDays(s, -7)), end: endOfMonth(addMonths(e, 1)) };
}

export function xOf(date: ISODate, rangeStart: ISODate, dayWidth: number): number {
  return diffDays(rangeStart, date) * dayWidth;
}

/** Inverse of xOf, rounded to the nearest day. */
export function dateAt(x: number, rangeStart: ISODate, dayWidth: number): ISODate {
  return addDays(rangeStart, Math.floor(x / dayWidth));
}

export interface HeaderCell {
  key: string;
  label: string;
  x: number;
  w: number;
  /** For day cells: weekend/holiday styling is applied by the view. */
  date?: ISODate;
  today?: boolean;
}

/** Two header rows: top (months or years) and bottom (days, weeks or months). */
export function headerTiers(
  range: { start: ISODate; end: ISODate },
  scale: GanttScale,
  now: ISODate,
  weekStart: 0 | 1,
): { top: HeaderCell[]; bottom: HeaderCell[] } {
  const dw = DAY_WIDTH[scale];
  const top: HeaderCell[] = [];
  const bottom: HeaderCell[] = [];
  const total = diffDays(range.start, range.end) + 1;
  const cell = (key: string, label: string, a: ISODate, b: ISODate, extra: Partial<HeaderCell> = {}) => {
    const x = Math.max(0, xOf(a, range.start, dw));
    const w = Math.min(total * dw, xOf(addDays(b, 1), range.start, dw)) - x;
    return { key, label, x, w, ...extra };
  };

  if (scale === 'month') {
    const [sy] = parts(range.start);
    const [ey] = parts(range.end);
    for (let y = sy; y <= ey; y++) top.push(cell(`y${y}`, `${y}年`, `${y}-01-01`, `${y}-12-31`));
  } else {
    for (let m = startOfMonth(range.start); m <= range.end; m = addMonths(m, 1)) {
      const [y, mm] = parts(m);
      top.push(cell(`m${m}`, `${y}年${mm}月`, m, endOfMonth(m)));
    }
  }

  if (scale === 'day') {
    for (let i = 0; i < total; i++) {
      const d = addDays(range.start, i);
      bottom.push({ key: d, label: String(parts(d)[2]), x: i * dw, w: dw, date: d, today: d === now });
    }
  } else if (scale === 'week') {
    for (let w = startOfWeek(range.start, weekStart); w <= range.end; w = addDays(w, 7)) {
      const [, m, d] = parts(w);
      const contains = dayNum(now) >= dayNum(w) && dayNum(now) < dayNum(w) + 7;
      bottom.push(cell(`w${w}`, `${m}/${d}`, w, addDays(w, 6), { today: contains }));
    }
  } else {
    for (let m = startOfMonth(range.start); m <= range.end; m = addMonths(m, 1)) {
      const mm = parts(m)[1];
      bottom.push(cell(`b${m}`, `${mm}月`, m, endOfMonth(m), { today: startOfMonth(now) === m }));
    }
  }
  return { top, bottom };
}

/** Label for the week header tooltip: "第 44 周". */
export function weekLabel(iso: ISODate): string {
  return `第 ${isoWeek(iso)} 周`;
}

/** SVG path for a finish-to-start dependency arrow between two bars. */
export function depPath(fromX: number, fromY: number, toX: number, toY: number): string {
  const gap = 8;
  if (toX - fromX >= gap * 2) {
    const mid = fromX + gap;
    return `M${fromX},${fromY} H${mid} V${toY} H${toX}`;
  }
  // Successor starts before the predecessor ends: route around below the source bar.
  const down = fromY + (toY > fromY ? 1 : -1) * (ROW_HEIGHT / 2);
  return `M${fromX},${fromY} H${fromX + gap} V${down} H${toX - gap} V${toY} H${toX}`;
}

/** True when `to` already (transitively) depends on `from`, i.e. adding from → to would loop. */
export function wouldCycle(tasks: Record<string, Task>, from: string, to: string): boolean {
  const seen = new Set<string>();
  const stack = [from];
  while (stack.length) {
    const id = stack.pop()!;
    if (id === to) return true;
    if (seen.has(id)) continue;
    seen.add(id);
    for (const d of tasks[id]?.deps ?? []) stack.push(d);
  }
  return false;
}

/** Whether `to` may get `from` as a new predecessor. */
export function canLink(tasks: Record<string, Task>, from: string, to: string): boolean {
  const a = tasks[from];
  const b = tasks[to];
  if (!a || !b || from === to || a.repeat || b.repeat) return false;
  if (b.deps?.includes(from)) return false;
  return !wouldCycle(tasks, from, to);
}

export function isWeekendCol(iso: ISODate, restDays: number[]): boolean {
  return restDays.includes(weekday(iso));
}
