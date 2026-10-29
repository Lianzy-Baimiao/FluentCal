<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { confirm, FButton, FSegmented, showMenu, type MenuItem } from '@/fluent';
import type { Task } from '@/types';
import { parts } from '@/lib/date';
import { saveGroup } from '@/lib/tasks';
import { doc, groupList, patchSettings } from '@/stores/doc';
import { now } from '@/composables/now';
import { removeTask } from '../menus';
import { openEditor, ui } from '../state';
import { centerDate, newGroup, scrollToDate, startRename, zoom, ensureRowVisible } from './actions';
import { dw, gs, HEAD, range, restSet, rows, scroller, selected, tableW, tiers, ungroupedCollapsed, chartW, bodyH } from './model';
import { dragTip } from './drag';
import { exportGantt } from './exportImage';
import { notify } from '../state';
import GanttTable from './GanttTable.vue';
import GanttChart from './GanttChart.vue';
import './gantt.css';

const el = ref<HTMLElement>();
const label = ref('');
/** Date at the viewport centre, captured on every scroll while dw and range are still the
 * ones the scroll position was made with (a watcher would already see the new day width). */
let center = '';
const updateLabel = () => {
  const [y, m] = parts(centerDate(0.3));
  label.value = `${y}年${m}月`;
  center = centerDate(0.5);
};

const scale = computed({ get: () => gs.value.scale, set: (v: string) => patchSettings({ gantt: { scale: v as 'day' } }) });
const scales = [
  { value: 'day', label: '日' },
  { value: 'week', label: '周' },
  { value: 'month', label: '月' },
];

function optionsMenu(e: MouseEvent) {
  const g = gs.value;
  const flip = (k: 'showTable' | 'showDeps' | 'hideDone' | 'hideSingleDay') => () => patchSettings({ gantt: { [k]: !g[k] } });
  const items: MenuItem[] = [
    { label: '显示任务表', checked: g.showTable, action: flip('showTable') },
    { label: '显示依赖连线', checked: g.showDeps, action: flip('showDeps') },
    { label: '隐藏已完成任务', checked: g.hideDone, action: flip('hideDone') },
    { label: '隐藏未分组的单日任务', checked: g.hideSingleDay, action: flip('hideSingleDay') },
    { separator: true },
    {
      label: '导出图片',
      icon: 'download',
      children: [
        { label: 'PNG 图片…', action: () => void saveImage('png') },
        { label: 'SVG 矢量图…', action: () => void saveImage('svg') },
      ],
    },
    {
      label: '只显示分组',
      icon: 'filter',
      children: [
        { label: '全部分组', radio: true, checked: !g.groupFilter.length, action: () => patchSettings({ gantt: { groupFilter: [] } }) },
        ...groupList.value.map((x) => ({ label: x.name, radio: true, checked: g.groupFilter.length === 1 && g.groupFilter[0] === x.id, action: () => patchSettings({ gantt: { groupFilter: [x.id] } }) })),
      ],
    },
  ];
  void showMenu(items, e.currentTarget as HTMLElement, { placement: 'bottom-end' });
}

async function saveImage(kind: 'png' | 'svg') {
  try {
    const path = await exportGantt(kind);
    if (path) notify(`已导出到 ${path}`);
  } catch (e) {
    notify(`导出失败：${e}`);
  }
}

// Keep the date under the viewport centre when zooming or when the range grows at the start.
watch([dw, () => range.value.start], () => {
  const keep = center;
  void nextTick(() => keep && scrollToDate(keep, 0.5));
});

function focusTask(id: string) {
  const t = doc.tasks[id];
  if (!t) return;
  const g = t.groupId ? doc.groups[t.groupId] : undefined;
  if (g?.collapsed) void saveGroup({ ...g, collapsed: false });
  if (!t.groupId) ungroupedCollapsed.value = false;
  if (gs.value.groupFilter.length && !gs.value.groupFilter.includes(t.groupId ?? '')) patchSettings({ gantt: { groupFilter: [] } });
  selected.value = id;
  void nextTick(() => {
    scrollToDate(t.start, 0.3);
    ensureRowVisible(`t:${id}`);
  });
}
watch(() => ui.ganttFocus, (id) => id && (focusTask(id), (ui.ganttFocus = '')));

/** A Gantt row is the whole series, so deleting a repeating task asks first. */
async function removeRow(t: Task) {
  if (t.repeat) {
    const r = await confirm({ title: '删除整个重复任务？', content: `“${t.title}”的所有重复都会被删除（可以用 Ctrl+Z 撤销）。`, primaryText: '全部删除', danger: true });
    if (r !== 'primary') return;
  }
  await removeTask(t);
}

function onWheel(e: WheelEvent) {
  if (!e.ctrlKey) return;
  e.preventDefault();
  zoom(e.deltaY < 0 ? 1 : -1);
}

function onKey(e: KeyboardEvent) {
  if ((e.target as HTMLElement).matches('input, textarea')) return;
  const t = doc.tasks[selected.value];
  const list = rows.value.flatMap((r) => (r.kind === 'task' ? [r.task.id] : []));
  const i = list.indexOf(selected.value);
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    const next = list[Math.max(0, Math.min(list.length - 1, i + (e.key === 'ArrowDown' ? 1 : -1)))];
    if (next) {
      selected.value = next;
      ensureRowVisible(`t:${next}`);
    }
  } else if (!t) return;
  else if (e.key === 'Delete') void removeRow(t);
  else if (e.key === 'Enter') openEditor({ mode: 'edit', taskId: t.id });
  else if (e.key === 'F2') void startRename('task', t.id);
  else return;
  e.preventDefault();
}

onMounted(() => {
  scroller.value = el.value ?? null;
  el.value?.addEventListener('wheel', onWheel, { passive: false });
  const focus = ui.ganttFocus;
  ui.ganttFocus = '';
  if (focus) focusTask(focus);
  else scrollToDate(now.value, 0.3);
  updateLabel();
});
onBeforeUnmount(() => {
  el.value?.removeEventListener('wheel', onWheel);
  scroller.value = null;
});
</script>

<template>
  <div class="gantt">
    <div class="g-toolbar">
      <span class="g-label">{{ label }}</span>
      <FSegmented v-model="scale" :options="scales" size="small" />
      <FButton variant="subtle" size="small" icon="today" @click="scrollToDate(now, 0.3, true)">今天</FButton>
      <span class="spacer" />
      <FButton variant="subtle" size="small" icon="add" @click="newGroup">新建分组</FButton>
      <FButton variant="subtle" size="icon" icon="filter" title="显示选项" @click="optionsMenu" />
    </div>
    <div ref="el" class="g-scroll" tabindex="0" aria-label="甘特图" @scroll.passive="updateLabel" @keydown="onKey">
      <div class="g-canvas" :style="{ width: `${tableW + chartW}px`, height: `${HEAD + bodyH}px` }">
        <div class="g-head" :style="{ height: `${HEAD}px` }">
          <div class="g-corner" :style="{ width: `${tableW}px` }">
            <span>任务</span>
          </div>
          <div class="g-tiers" :style="{ width: `${chartW}px` }">
            <div class="tier top">
              <div v-for="c in tiers.top" :key="c.key" class="hc" :style="{ left: `${c.x}px`, width: `${c.w}px` }">
                <span class="sticky" :style="{ left: `${tableW + 6}px` }">{{ c.label }}</span>
              </div>
            </div>
            <div class="tier bottom" :class="gs.scale">
              <div
                v-for="c in tiers.bottom"
                :key="c.key"
                class="hc"
                :class="{ today: c.today, rest: !!c.date && restSet.has(c.date) }"
                :style="{ left: `${c.x}px`, width: `${c.w}px` }"
              >
                <span>{{ c.label }}</span>
              </div>
            </div>
          </div>
        </div>
        <div class="g-body">
          <GanttTable v-if="gs.showTable" />
          <GanttChart />
        </div>
      </div>
      <div v-if="!rows.length" class="g-empty">
        <p>还没有可显示的任务。</p>
        <FButton variant="accent" icon="add" @click="newGroup">新建分组</FButton>
      </div>
    </div>
    <div v-if="dragTip.show" class="g-dragtip" :style="{ left: `${dragTip.x}px`, top: `${dragTip.y}px` }">{{ dragTip.text }}</div>
  </div>
</template>
