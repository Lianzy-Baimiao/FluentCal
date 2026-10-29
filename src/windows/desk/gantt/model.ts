import { computed, ref, shallowRef } from 'vue';
import { taskColor } from '@/fluent';
import type { ISODate, Task } from '@/types';
import { addDays, diffDays } from '@/lib/date';
import { isRestDay, legalHoliday } from '@/lib/lunar';
import { occurrences } from '@/lib/recur';
import { buildRows, chartRange, DAY_WIDTH, depPath, headerTiers, ROW_HEIGHT, UNGROUPED, xOf, type GanttRow } from '@/lib/gantt';
import { doc, groupList, taskList } from '@/stores/doc';
import { now } from '@/composables/now';
import { isDark } from '@/composables/theme';

export const RH = ROW_HEIGHT;
export const HEAD = 44;

export const gs = computed(() => doc.settings.gantt);
export const dw = computed(() => DAY_WIDTH[gs.value.scale]);
export const tableW = computed(() => (gs.value.showTable ? Math.max(140, Math.min(480, gs.value.tableWidth)) : 0));

/** Elements registered by the view components. */
export const scroller = shallowRef<HTMLElement | null>(null);
export const chartEl = shallowRef<HTMLElement | null>(null);
export const selected = ref('');
export const ungroupedCollapsed = ref(false);

export const rows = computed<GanttRow[]>(() => {
  const g = gs.value;
  const all = buildRows(taskList.value, groupList.value, { hideDone: g.hideDone, hideSingleDay: g.hideSingleDay, groupFilter: g.groupFilter });
  return ungroupedCollapsed.value ? all.filter((r) => r.kind === 'group' || r.groupId !== UNGROUPED) : all;
});

export const range = computed(() => {
  const r = chartRange(taskList.value.filter((t) => !t.repeat), now.value, gs.value.scale);
  const lo = addDays(now.value, -3 * 366);
  const hi = addDays(now.value, 5 * 366);
  return { start: r.start < lo ? lo : r.start, end: r.end > hi ? hi : r.end };
});
export const days = computed(() => diffDays(range.value.start, range.value.end) + 1);
export const chartW = computed(() => days.value * dw.value);
export const bodyH = computed(() => Math.max(rows.value.length, 1) * RH);
export const xOfDate = (d: ISODate) => xOf(d, range.value.start, dw.value);
export const tiers = computed(() => headerTiers(range.value, gs.value.scale, now.value, doc.settings.calendar.weekStart));
export const restDays = computed(() => doc.settings.calendar.restDays);

/** Runs of consecutive rest days (weekends, holidays) for the column shading. */
export const stripes = computed(() => {
  if (gs.value.scale === 'month') return [];
  const out: { key: string; x: number; w: number; holiday: boolean }[] = [];
  let run: { i: number; n: number; holiday: boolean } | null = null;
  const flush = () => run && out.push({ key: `s${run.i}`, x: run.i * dw.value, w: run.n * dw.value, holiday: run.holiday });
  for (let i = 0; i < days.value; i++) {
    const d = addDays(range.value.start, i);
    if (!isRestDay(d, restDays.value)) {
      flush();
      run = null;
      continue;
    }
    const holiday = legalHoliday(d)?.work === false;
    if (run && run.holiday === holiday && run.i + run.n === i) run.n++;
    else {
      flush();
      run = { i, n: 1, holiday };
    }
  }
  flush();
  return out;
});

/** Rest days in the range, for header day styling. */
export const restSet = computed(() => {
  const set = new Set<ISODate>();
  for (const st of stripes.value) {
    const first = Math.round(st.x / dw.value);
    for (let i = 0; i < Math.round(st.w / dw.value); i++) set.add(addDays(range.value.start, first + i));
  }
  return set;
});

export const monthLines = computed(() =>
  (gs.value.scale === 'month' ? tiers.value.bottom : tiers.value.top).map((c) => c.x).filter((x) => x > 0),
);

export interface Bar {
  key: string;
  task: Task;
  row: number;
  date: ISODate;
  start: ISODate;
  end: ISODate;
  x: number;
  w: number;
  progress: number;
  done: boolean;
  rep: boolean;
  bar: string;
  onBar: string;
  text: string;
}

/** Bar geometry while dragging, before it is saved. */
export const preview = ref<{ id: string; start: ISODate; end: ISODate; progress: number } | null>(null);

export function barColors(t: Task) {
  const g = t.groupId ? doc.groups[t.groupId] : undefined;
  return taskColor(t.color || g?.color || undefined, isDark.value);
}

const baseBars = computed<Bar[]>(() => {
  const out: Bar[] = [];
  const { start: rs, end: re } = range.value;
  rows.value.forEach((r, row) => {
    if (r.kind !== 'task') return;
    const t = r.task;
    const c = barColors(t);
    // The range is at most ~8 years; 3000 covers a daily repeat across it.
    const occ = t.repeat ? occurrences(t, rs, re, 3000) : [{ date: t.start, start: t.start, end: t.end, done: t.done }];
    for (const o of occ) {
      out.push({
        key: t.repeat ? `${t.id}@${o.date}` : t.id,
        task: t,
        row,
        date: o.date,
        start: o.start,
        end: o.end,
        x: xOfDate(o.start),
        w: (diffDays(o.start, o.end) + 1) * dw.value,
        progress: o.done ? 100 : t.repeat ? 0 : t.progress,
        done: o.done,
        rep: !!t.repeat,
        bar: c.bar,
        onBar: c.onBar,
        text: c.text,
      });
    }
  });
  return out;
});

export const bars = computed<Bar[]>(() => {
  const p = preview.value;
  if (!p) return baseBars.value;
  return baseBars.value.map((b) =>
    b.key !== p.id
      ? b
      : { ...b, start: p.start, end: p.end, progress: p.progress, x: xOfDate(p.start), w: (diffDays(p.start, p.end) + 1) * dw.value },
  );
});

export const summaries = computed(() =>
  rows.value.flatMap((r, row) =>
    r.kind === 'group' && r.start && r.end
      ? [{ key: r.key, row, x: xOfDate(r.start), w: (diffDays(r.start, r.end) + 1) * dw.value, progress: r.progress, color: taskColor(r.group.color, isDark.value).bar }]
      : [],
  ),
);

/** Finish-to-start arrows; `late` when the successor starts before its predecessor ends. */
export const links = computed(() => {
  if (!gs.value.showDeps) return [];
  const pos = new Map<string, Bar>();
  for (const b of bars.value) if (!b.rep) pos.set(b.task.id, b);
  const out: { key: string; d: string; late: boolean }[] = [];
  for (const b of pos.values()) {
    for (const dep of b.task.deps ?? []) {
      const p = pos.get(dep);
      if (!p) continue;
      out.push({ key: `${dep}>${b.task.id}`, d: depPath(p.x + p.w, p.row * RH + RH / 2, b.x, b.row * RH + RH / 2), late: p.end >= b.start });
    }
  }
  return out;
});
