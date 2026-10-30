import { nextTick, ref } from 'vue';
import { confirm, showMenu, type MenuItem } from '@/fluent';
import type { GanttScale, Group, ISODate } from '@/types';
import { addDays } from '@/lib/date';
import { UNGROUPED, UNGROUPED_GROUP } from '@/lib/gantt';
import { addTask, deleteGroup, makeGroup, saveGroup, saveTask } from '@/lib/tasks';
import { doc, groupList, patchSettings, put, storeState } from '@/stores/doc';
import { now } from '@/composables/now';
import { colorSwatches } from '../menus';
import { openEditor } from '../state';
import { dw, gs, HEAD, range, RH, rows, scroller, selected, tableW, ungroupedCollapsed, xOfDate } from './model';

/** Inline rename in the table: `task:<id>` or `group:<id>`. */
export const editing = ref('');
export const editText = ref('');

export async function startRename(kind: 'task' | 'group', id: string) {
  if (storeState.readOnly) {
    if (kind === 'task') openEditor({ mode: 'edit', taskId: id }); // the web view's details
    return;
  }
  if (!gs.value.showTable) {
    if (kind === 'task') openEditor({ mode: 'edit', taskId: id });
    return;
  }
  editing.value = `${kind}:${id}`;
  editText.value = kind === 'task' ? (doc.tasks[id]?.title ?? '') : (doc.groups[id]?.name ?? '');
  await nextTick();
  const el = scroller.value?.querySelector<HTMLInputElement>('input.rename');
  el?.focus();
  el?.select();
}

export function commitRename() {
  const [kind, id] = editing.value.split(':');
  const v = editText.value.trim();
  editing.value = '';
  if (!v) return;
  if (kind === 'task' && doc.tasks[id] && doc.tasks[id].title !== v) void saveTask({ ...doc.tasks[id], title: v });
  if (kind === 'group' && doc.groups[id] && doc.groups[id].name !== v) void saveGroup({ ...doc.groups[id], name: v });
}

export function scrollToDate(d: ISODate, frac = 0.3, smooth = false) {
  const el = scroller.value;
  if (!el) return;
  const left = Math.max(0, xOfDate(d) - (el.clientWidth - tableW.value) * frac);
  el.scrollTo({ left, behavior: smooth ? 'smooth' : 'auto' });
}

/** Scrolls horizontally only when the date is outside the visible chart area. */
export function revealDate(d: ISODate) {
  const el = scroller.value;
  if (!el) return;
  const x = xOfDate(d);
  if (x < el.scrollLeft || x > el.scrollLeft + el.clientWidth - tableW.value - 40) scrollToDate(d, 0.2, true);
}

export function ensureRowVisible(key: string) {
  const el = scroller.value;
  const i = rows.value.findIndex((r) => r.key === key);
  if (!el || i < 0) return;
  const top = HEAD + i * RH;
  if (top < el.scrollTop + HEAD) el.scrollTop = top - HEAD - RH;
  else if (top + RH > el.scrollTop + el.clientHeight) el.scrollTop = top + RH * 2 - el.clientHeight;
}

/** Creates a task in a group and starts renaming it. Ungrouped tasks get ≥2 days so they stay visible. */
export async function createTask(groupId: string, start: ISODate, end: ISODate, milestone = false) {
  const gid = groupId === UNGROUPED ? undefined : groupId;
  if (!gid && !milestone && end === start) end = addDays(start, 1);
  const t = await addTask({ title: milestone ? '新里程碑' : '新任务', start, end: milestone ? start : end, groupId: gid, milestone: milestone || undefined });
  selected.value = t.id;
  await nextTick();
  ensureRowVisible(`t:${t.id}`);
  void startRename('task', t.id);
}

/** "Add task" row: a few days right after the group's last task (or today). */
export function addInGroup(groupId: string, milestone = false) {
  if (storeState.readOnly) return;
  const list = Object.values(doc.tasks).filter((t) => (t.groupId ?? UNGROUPED) === groupId);
  const last = list.reduce<ISODate>((m, t) => (t.end > m ? t.end : m), '');
  const start = last ? addDays(last, 1) : now.value;
  const len = gs.value.scale === 'day' ? 2 : gs.value.scale === 'week' ? 6 : 13;
  void createTask(groupId, start, addDays(start, len), milestone);
}

export async function newGroup() {
  if (storeState.readOnly) return;
  const palette = ['blue', 'purple', 'teal', 'orange', 'pink', 'green', 'indigo', 'red'];
  const g = makeGroup('新分组', palette[groupList.value.length % palette.length]);
  await saveGroup(g);
  if (gs.value.groupFilter.length) patchSettings({ gantt: { groupFilter: [] } });
  await nextTick();
  ensureRowVisible(`g:${g.id}`);
  void startRename('group', g.id);
}

export function toggleCollapse(g: Group) {
  if (g.id === UNGROUPED) ungroupedCollapsed.value = !ungroupedCollapsed.value;
  // Web view: only in this page (collapsing is a way of looking, not an edit).
  else if (storeState.readOnly) doc.groups[g.id] = { ...g, collapsed: !g.collapsed };
  else void saveGroup({ ...g, collapsed: !g.collapsed });
}

function moveGroup(g: Group, dir: -1 | 1) {
  const list = groupList.value;
  const other = list[list.findIndex((x) => x.id === g.id) + dir];
  if (other) void put('groups', { [g.id]: { ...g, order: other.order }, [other.id]: { ...other, order: g.order } });
}

export function groupMenu(g: Group, at: { x: number; y: number } | DOMRect) {
  if (storeState.readOnly) return;
  const real = g.id !== UNGROUPED;
  const items: (MenuItem | false)[] = [
    { label: '添加任务', icon: 'add', action: () => addInGroup(g.id) },
    { label: '添加里程碑', icon: 'flag', action: () => addInGroup(g.id, true) },
    real && { separator: true },
    real && { label: '重命名', icon: 'edit', shortcut: 'F2', action: () => void startRename('group', g.id) },
    real && { swatches: colorSwatches(g.color, false), onSwatch: (k) => void saveGroup({ ...g, color: k }) },
    real && { label: '上移', icon: 'chevronUp', action: () => moveGroup(g, -1) },
    real && { label: '下移', icon: 'chevronDown', action: () => moveGroup(g, 1) },
    real && { separator: true },
    real && {
      label: '删除分组',
      icon: 'delete',
      danger: true,
      action: async () => {
        const n = Object.values(doc.tasks).filter((t) => t.groupId === g.id).length;
        const r = await confirm({ title: `删除分组“${g.name}”？`, content: n ? `组内 ${n} 个任务会移到“未分组”，不会被删除。` : undefined, primaryText: '删除', danger: true });
        if (r === 'primary') await deleteGroup(g.id);
      },
    },
  ];
  return showMenu(items.filter(Boolean) as MenuItem[], at);
}

export function groupOf(id: string): Group {
  return doc.groups[id] ?? UNGROUPED_GROUP;
}

const SCALES: GanttScale[] = ['month', 'week', 'day'];
export function zoom(dir: 1 | -1) {
  const i = SCALES.indexOf(gs.value.scale) + dir;
  if (i >= 0 && i < SCALES.length) patchSettings({ gantt: { scale: SCALES[i] } });
}

/** Date at a fraction of the chart viewport width (0.5 = centre). */
export function centerDate(frac = 0.5): ISODate {
  const el = scroller.value;
  if (!el) return now.value;
  const x = el.scrollLeft + (el.clientWidth - tableW.value) * frac;
  return addDays(range.value.start, Math.floor(x / dw.value));
}
