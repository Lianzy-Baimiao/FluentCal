<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue';
import { TASK_COLORS } from '@/fluent';
import type { ISODate } from '@/types';
import { addDays, addMonths, diffDays, formatYMD, isoWeek, monthWeeks, parts, startOfMonth, weekday, WEEKDAY_SHORT } from '@/lib/date';
import { almanac, dayInfo } from '@/lib/lunar';
import { occurrences, type Occurrence } from '@/lib/recur';
import { layoutWeek, type Chip } from '@/lib/layout';
import { moveOccurrence, setOccurrenceDone } from '@/lib/tasks';
import { doc, taskList } from '@/stores/doc';
import { now } from '@/composables/now';
import { defaultWeeksStart, goToday, openEditor, page, ui } from './state';
import { cellMenu, chipMenu, removeTask } from './menus';
import type { CellInfo } from './types';
import WeekRow from './WeekRow.vue';
import DayListFlyout from './DayListFlyout.vue';

const props = defineProps<{ mode: 'month' | 'weeks' }>();

const cal = computed(() => doc.settings.calendar);

const weeks = computed<ISODate[]>(() => {
  if (props.mode === 'weeks') {
    const start = ui.weeksStart || defaultWeeksStart();
    const n = Math.max(2, Math.min(10, cal.value.weeksRows));
    return Array.from({ length: n }, (_, i) => addDays(start, i * 7));
  }
  const [y, m] = parts(ui.cursor);
  return monthWeeks(y, m, cal.value.weekStart, cal.value.monthRows);
});
const rangeStart = computed(() => weeks.value[0]);
const rangeEnd = computed(() => addDays(weeks.value[weeks.value.length - 1], 6));
const focusMonth = computed(() => (props.mode === 'month' ? ui.cursor.slice(0, 7) : ''));

const occs = computed<Occurrence[]>(() => {
  const show = doc.settings.tasks.showCompleted;
  const out: Occurrence[] = [];
  for (const t of taskList.value) {
    for (const o of occurrences(t, rangeStart.value, rangeEnd.value)) if (show || !o.done) out.push(o);
  }
  return out;
});

// Sizes scale with the font setting; the lane capacity follows the measured row height.
const fs = computed(() => doc.settings.desk.fontScale / 100);
const headH = computed(() => Math.round(22 * fs.value));
const chipH = computed(() => Math.round(17 * fs.value));
const laneH = computed(() => chipH.value + doc.settings.desk.lineSpacing);
const gridEl = ref<HTMLElement>();
const rowH = ref(80);
function measure() {
  const el = gridEl.value;
  if (!el) return;
  const n = weeks.value.length;
  rowH.value = (el.clientHeight - doc.settings.desk.cellGap * (n - 1)) / n;
}
const capacity = computed(() =>
  Math.max(0, Math.floor((rowH.value - headH.value - 3 + doc.settings.desk.lineSpacing) / laneH.value)),
);
const layouts = computed(() =>
  weeks.value.map((w) =>
    layoutWeek(w, occs.value, { capacity: capacity.value, completedLast: doc.settings.tasks.completedLast }),
  ),
);

function markColor(key: string | undefined): string | undefined {
  if (!key) return;
  return TASK_COLORS.find((c) => c.key === key)?.bar ?? key;
}

function cellInfo(date: ISODate): CellInfo {
  const info = dayInfo(date);
  const [, m, d] = parts(date);
  const c = cal.value;
  const legal = c.showHolidays ? info.legal : undefined;
  const rest = legal ? !legal.work : c.restDays.includes(weekday(date));
  const lunar = c.showLunar || info.kind === 'festival' ? info.text : '';
  const wd = `周${WEEKDAY_SHORT[weekday(date)]}`;
  const tip = [`${formatYMD(date)} ${wd}`, `农历${info.lunarLabel} · ${info.ganzhiYear}`];
  if (info.festivals.length) tip.push(info.festivals.join('、'));
  if (info.term) tip.push(`节气：${info.term}`);
  if (legal) tip.push(`${legal.name} · ${legal.work ? '调休上班' : '放假'}`);
  if (c.showAlmanac) {
    const a = almanac(date);
    tip.push(`宜：${a.recommends.slice(0, 8).join(' ') || '—'}`, `忌：${a.avoids.slice(0, 8).join(' ') || '—'}`);
  }
  const count = occs.value.filter((o) => o.start <= date && o.end >= date).length;
  return {
    date,
    label: d === 1 && date.slice(0, 7) !== focusMonth.value ? `${m}月` : String(d),
    lunar,
    lunarKind: info.kind,
    badge: legal ? (legal.work ? '班' : '休') : undefined,
    rest,
    today: date === now.value,
    dim: props.mode === 'month' && date.slice(0, 7) !== focusMonth.value,
    mark: markColor(doc.dayMarks[date]?.color),
    tip: tip.join('\n'),
    aria: `${m}月${d}日 ${wd}，农历${info.lunarLabel}${legal ? `，${legal.work ? '调休上班' : '放假'}` : ''}，${count ? `${count} 项任务` : '无任务'}`,
  };
}

const cells = computed(() => weeks.value.map((w) => Array.from({ length: 7 }, (_, i) => cellInfo(addDays(w, i)))));
const weekNos = computed(() =>
  cal.value.showWeekNumbers ? weeks.value.map((w) => isoWeek(addDays(w, cal.value.weekStart === 0 ? 1 : 0))) : [],
);
const headers = computed(() =>
  Array.from({ length: 7 }, (_, i) => {
    const wd = (cal.value.weekStart + i) % 7;
    return { label: WEEKDAY_SHORT[wd], rest: cal.value.restDays.includes(wd) };
  }),
);

// ---------------------------------------------------------------- chips

const toggle = (c: Chip) => void setOccurrenceDone(c.occ.task, c.occ.date, !c.occ.done);
const open = (c: Chip) => openEditor({ mode: 'edit', taskId: c.occ.task.id, date: c.occ.date });
const menu = (c: Chip, e: MouseEvent) => void chipMenu(c.occ, { x: e.clientX, y: e.clientY });

function chipKey(c: Chip, e: KeyboardEvent) {
  const el = e.currentTarget as HTMLElement;
  if (e.key === 'Enter') open(c);
  else if (e.key === ' ') toggle(c);
  else if (e.key === 'Delete') void removeTask(c.occ.task, c.occ.date, 'this'); else if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) void chipMenu(c.occ, el.getBoundingClientRect());
  else return;
  e.preventDefault();
  e.stopPropagation();
}

// Drag a chip onto another day. The grab offset keeps a multi-day bar under the pointer.
const drag = reactive({ active: false, key: '', over: '' as ISODate, x: 0, y: 0, label: '' });
let ds: { c: Chip; x: number; y: number; offset: number; el: HTMLElement; id: number } | null = null;

function dateAt(x: number, y: number): ISODate | undefined {
  for (const el of document.elementsFromPoint(x, y)) {
    const d = (el as HTMLElement).dataset?.date;
    if (d && gridEl.value?.contains(el)) return d;
  }
}

function chipDown(c: Chip, e: PointerEvent) {
  if (e.button !== 0) return;
  const at = dateAt(e.clientX, e.clientY);
  ds = { c, x: e.clientX, y: e.clientY, offset: at ? diffDays(c.occ.start, at) : 0, el: e.currentTarget as HTMLElement, id: e.pointerId };
  window.addEventListener('pointermove', onMove);
  window.addEventListener('pointerup', onUp);
  window.addEventListener('keydown', onDragKey, true);
}

function onMove(e: PointerEvent) {
  if (!ds) return;
  if (!drag.active) {
    if (Math.hypot(e.clientX - ds.x, e.clientY - ds.y) < 5) return;
    Object.assign(drag, { active: true, key: ds.c.key, label: ds.c.occ.task.title });
    try {
      ds.el.setPointerCapture(ds.id);
    } catch {
      /* element may be gone */
    }
  }
  drag.x = e.clientX;
  drag.y = e.clientY;
  drag.over = dateAt(e.clientX, e.clientY) ?? '';
}

function endDrag() {
  window.removeEventListener('pointermove', onMove);
  window.removeEventListener('pointerup', onUp);
  window.removeEventListener('keydown', onDragKey, true);
  Object.assign(drag, { active: false, key: '', over: '' });
  ds = null;
}

function onUp(e: PointerEvent) {
  const s = ds;
  const wasActive = drag.active;
  const target = wasActive ? dateAt(e.clientX, e.clientY) : undefined;
  endDrag();
  if (s && target) void moveOccurrence(s.c.occ, addDays(target, -s.offset));
}

function onDragKey(e: KeyboardEvent) {
  if (e.key === 'Escape' && drag.active) {
    e.stopPropagation();
    endDrag();
  }
}

// ---------------------------------------------------------------- grid keyboard, wheel

function select(d: ISODate) {
  ui.selected = d;
  if (props.mode === 'month') {
    if (d < rangeStart.value || d > rangeEnd.value) ui.cursor = startOfMonth(d);
  } else if (d < rangeStart.value) page(-1);
  else if (d > rangeEnd.value) page(1);
  void nextTick(() => gridEl.value?.querySelector<HTMLElement>(`.cell[data-date="${d}"]`)?.focus());
}

function onGridKey(e: KeyboardEvent) {
  const target = e.target as HTMLElement;
  if (!target.classList.contains('cell')) return;
  const moves: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 };
  if (e.key in moves) select(addDays(ui.selected, moves[e.key]));
  else if (e.key === 'Enter') ui.quickAdd = { date: ui.selected };
  else if (e.key === 'Home') {
    goToday();
    select(now.value);
  } else if (e.key === 'PageUp' || e.key === 'PageDown') {
    const dir = e.key === 'PageUp' ? -1 : 1;
    if (props.mode === 'month') select(addMonths(ui.selected, dir));
    else select(addDays(ui.selected, dir * 7));
  } else if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) void cellMenu(ui.selected, target.getBoundingClientRect());
  else return;
  e.preventDefault();
}

let wheelAcc = 0;
let wheelAt = 0;
function onWheel(e: WheelEvent) {
  if (e.ctrlKey || drag.active) return;
  wheelAcc += e.deltaY;
  const t = performance.now();
  if (Math.abs(wheelAcc) >= 50 && t - wheelAt > 200) {
    page(wheelAcc > 0 ? 1 : -1);
    wheelAcc = 0;
    wheelAt = t;
  }
}

// ---------------------------------------------------------------- "+N" day list

const moreOpen = ref(false);
const moreDate = ref<ISODate>('');
const moreAnchor = ref<HTMLElement | null>(null);
function onMore(date: ISODate, el: HTMLElement) {
  moreDate.value = date;
  moreAnchor.value = el;
  moreOpen.value = true;
}

let ro: ResizeObserver | undefined;
onMounted(() => {
  ro = new ResizeObserver(measure);
  if (gridEl.value) ro.observe(gridEl.value);
  measure();
});
onBeforeUnmount(() => {
  ro?.disconnect();
  endDrag();
});
watch(
  () => [weeks.value.length, doc.settings.desk.cellGap],
  () => void nextTick(measure),
);
</script>

<template>
  <div class="month" @wheel.passive="onWheel">
    <div class="weekdays" aria-hidden="true">
      <span v-if="cal.showWeekNumbers" class="wn">周</span>
      <div class="wd-grid">
        <span v-for="(h, i) in headers" :key="i" :class="{ rest: h.rest }">{{ h.label }}</span>
      </div>
    </div>
    <div
      ref="gridEl"
      class="grid"
      role="grid"
      :aria-label="mode === 'month' ? `${ui.cursor.slice(0, 4)}年${+ui.cursor.slice(5, 7)}月` : '多周视图'"
      :style="{ gridTemplateRows: `repeat(${weeks.length}, minmax(0, 1fr))` }"
      @keydown="onGridKey"
    >
      <WeekRow
        v-for="(w, i) in weeks"
        :key="w"
        :cells="cells[i]"
        :layout="layouts[i]"
        :chip-h="chipH"
        :lane-h="laneH"
        :head-h="headH"
        :week-no="weekNos[i]"
        :drop-date="drag.over"
        :dragging-key="drag.key"
        @chip-toggle="toggle"
        @chip-open="open"
        @chip-menu="menu"
        @chip-down="chipDown"
        @chip-key="chipKey"
        @cell-menu="(d, e) => cellMenu(d, { x: e.clientX, y: e.clientY })"
        @more="onMore"
      />
    </div>
    <div v-if="drag.active" class="ghost" :style="{ left: `${drag.x + 12}px`, top: `${drag.y + 10}px` }">
      {{ drag.label }}
    </div>
    <DayListFlyout v-model:open="moreOpen" :date="moreDate" :anchor="moreAnchor" />
  </div>
</template>

<style scoped>
.month {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: var(--cell-gap);
}
.weekdays {
  display: flex;
  gap: var(--cell-gap);
  font-size: calc(12px * var(--fs));
  color: var(--text-secondary);
  text-shadow: var(--text-glow);
}
.wn {
  width: 18px;
  flex: none;
  text-align: center;
  font-size: calc(10px * var(--fs));
  color: var(--text-tertiary);
}
.wd-grid {
  flex: 1;
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  gap: var(--cell-gap);
}
.wd-grid span {
  padding: 0 6px;
}
.wd-grid .rest {
  color: var(--holiday-text);
}
.grid {
  flex: 1;
  min-height: 0;
  display: grid;
  gap: var(--cell-gap);
}
.ghost {
  position: fixed;
  z-index: 50;
  pointer-events: none;
  max-width: 220px;
  padding: 3px 8px;
  border-radius: 4px;
  background: var(--accent-fill);
  color: var(--text-on-accent);
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  box-shadow: var(--shadow-flyout);
}
</style>
