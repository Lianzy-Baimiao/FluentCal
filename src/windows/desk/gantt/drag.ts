import { reactive } from 'vue';
import type { ISODate } from '@/types';
import { addDays, diffDays } from '@/lib/date';
import { canLink } from '@/lib/gantt';
import { saveTask } from '@/lib/tasks';
import { doc, storeState } from '@/stores/doc';
import { openEditor } from '../state';
import { createTask } from './actions';
import { chartEl, dw, preview, range, RH, selected, type Bar } from './model';

type Mode = 'move' | 'start' | 'end' | 'progress';

/** Floating label shown while dragging (dates or percentage). */
export const dragTip = reactive({ show: false, x: 0, y: 0, text: '' });
/** Dashed bar shown while drag-creating a task on an "add" row. */
export const ghost = reactive({ show: false, row: 0, start: '' as ISODate, end: '' as ISODate });

let s: { bar: Bar; mode: Mode; x0: number; moved: boolean } | null = null;

function listen(move: (e: PointerEvent) => void, up: (e: PointerEvent) => void) {
  const stop = () => {
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', onUp);
    window.removeEventListener('keydown', onKey, true);
  };
  const onUp = (e: PointerEvent) => {
    stop();
    up(e);
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key !== 'Escape') return;
    e.stopPropagation();
    stop();
    preview.value = null;
    dragTip.show = false;
    ghost.show = false;
    linkDrag.show = false;
    s = null;
  };
  window.addEventListener('pointermove', move);
  window.addEventListener('pointerup', onUp);
  window.addEventListener('keydown', onKey, true);
}

function tipAt(e: PointerEvent, text: string) {
  Object.assign(dragTip, { show: true, x: e.clientX + 14, y: e.clientY + 16, text });
}

const span = (a: ISODate, b: ISODate) => `${a.slice(5).replace('-', '/')} – ${b.slice(5).replace('-', '/')}（${diffDays(a, b) + 1} 天）`;

export function onBarDown(bar: Bar, e: PointerEvent) {
  if (e.button !== 0) return;
  selected.value = bar.task.id;
  if (storeState.readOnly) return onBarDblClick(bar); // web view: a tap shows the details
  if (bar.rep) return; // repeating instances are edited as a series in the editor
  const role = (e.target as HTMLElement).dataset.handle as Mode | undefined;
  const mode: Mode = bar.task.milestone ? 'move' : (role ?? 'move');
  s = { bar, mode, x0: e.clientX, moved: false };
  listen(onMove, onUp);
}

function onMove(e: PointerEvent) {
  if (!s) return;
  const dx = e.clientX - s.x0;
  if (!s.moved && Math.abs(dx) < 3) return;
  s.moved = true;
  const t = s.bar.task;
  if (s.mode === 'progress') {
    const p = Math.max(0, Math.min(100, Math.round((t.progress + (dx / Math.max(1, s.bar.w)) * 100) / 5) * 5));
    preview.value = { id: s.bar.key, start: t.start, end: t.end, progress: p };
    tipAt(e, `进度 ${p}%`);
    return;
  }
  const dd = Math.round(dx / dw.value);
  let start = t.start;
  let end = t.end;
  if (s.mode === 'move') {
    start = addDays(t.start, dd);
    end = addDays(t.end, dd);
  } else if (s.mode === 'start') start = addDays(t.start, Math.min(dd, diffDays(t.start, t.end)));
  else end = addDays(t.end, Math.max(dd, -diffDays(t.start, t.end)));
  preview.value = { id: s.bar.key, start, end, progress: t.progress };
  tipAt(e, t.milestone ? start : span(start, end));
}

function onUp() {
  const st = s;
  const p = preview.value;
  s = null;
  dragTip.show = false;
  preview.value = null;
  if (!st || !st.moved || !p) return;
  const t = doc.tasks[st.bar.task.id];
  if (!t) return;
  if (st.mode === 'progress') {
    if (p.progress !== t.progress) void saveTask({ ...t, progress: p.progress, done: p.progress >= 100 });
  } else if (p.start !== t.start || p.end !== t.end) void saveTask({ ...t, start: p.start, end: p.end });
}

export function onBarDblClick(bar: Bar) {
  openEditor({ mode: 'edit', taskId: bar.task.id, date: bar.date });
}

/** Drag on an "add" row to create a task spanning the dragged days; a click creates a short one. */
export function onAddRowDown(groupId: string, row: number, e: PointerEvent) {
  if (e.button !== 0 || !chartEl.value || storeState.readOnly) return;
  const left = chartEl.value.getBoundingClientRect().left;
  const at = (x: number) => addDays(range.value.start, Math.floor((x - left) / dw.value));
  const d0 = at(e.clientX);
  Object.assign(ghost, { show: true, row, start: d0, end: d0 });
  listen(
    (ev) => {
      const d = at(ev.clientX);
      ghost.start = d < d0 ? d : d0;
      ghost.end = d < d0 ? d0 : d;
      tipAt(ev, span(ghost.start, ghost.end));
    },
    () => {
      const { start, end } = ghost;
      ghost.show = false;
      dragTip.show = false;
      void createTask(groupId, start, end === start ? addDays(start, dw.value >= 32 ? 2 : 6) : end);
    },
  );
}

export const rowTop = (row: number) => row * RH;

/** Rubber band while dragging from a bar's link handle to another bar. */
export const linkDrag = reactive({ show: false, x1: 0, y1: 0, x2: 0, y2: 0, ok: false });

function barUnder(x: number, y: number): string | undefined {
  for (const el of document.elementsFromPoint(x, y)) {
    const id = (el as HTMLElement).dataset?.task;
    if (id) return id;
  }
}

/** Drag from the end of `bar` onto another bar: that task gets `bar` as predecessor. */
export function onLinkDown(bar: Bar, e: PointerEvent) {
  if (e.button !== 0 || !chartEl.value || storeState.readOnly) return;
  e.stopPropagation();
  const from = bar.task.id;
  const rect = () => chartEl.value!.getBoundingClientRect();
  Object.assign(linkDrag, { show: true, x1: bar.x + bar.w, y1: bar.row * RH + RH / 2, x2: bar.x + bar.w, y2: bar.row * RH + RH / 2, ok: false });
  listen(
    (ev) => {
      const r = rect();
      const to = barUnder(ev.clientX, ev.clientY);
      Object.assign(linkDrag, { x2: ev.clientX - r.left, y2: ev.clientY - r.top, ok: !!to && canLink(doc.tasks, from, to) });
      tipAt(ev, to && to !== from ? (linkDrag.ok ? `设为“${doc.tasks[to]?.title}”的前置任务` : '不能连接（会形成循环或已连接）') : '拖到另一个任务上');
    },
    (ev) => {
      linkDrag.show = false;
      dragTip.show = false;
      const to = barUnder(ev.clientX, ev.clientY);
      const t = to ? doc.tasks[to] : undefined;
      if (t && canLink(doc.tasks, from, t.id)) void saveTask({ ...t, deps: [...(t.deps ?? []), from] });
    },
  );
}
