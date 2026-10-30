<script setup lang="ts">
import { computed } from 'vue';
import { FIcon, taskColor } from '@/fluent';
import type { Task } from '@/types';
import { diffDays, parts } from '@/lib/date';
import { occurrenceOn } from '@/lib/recur';
import { setOccurrenceDone } from '@/lib/tasks';
import { isDark } from '@/composables/theme';
import { chipMenu } from '../menus';
import { openEditor } from '../state';
import { addInGroup, commitRename, editing, editText, groupMenu, revealDate, startRename, toggleCollapse } from './actions';
import { rows, selected, tableW, ungroupedCollapsed } from './model';
import { patchSettings, storeState } from '@/stores/doc';
import { UNGROUPED } from '@/lib/gantt';

const showDates = computed(() => tableW.value >= 230);
const showPct = computed(() => tableW.value >= 180);
const md = (d: string) => `${parts(d)[1]}/${parts(d)[2]}`;
const dates = (t: Task) => (t.start === t.end ? md(t.start) : `${md(t.start)}–${md(t.end)}`);
const collapsed = (id: string, c?: boolean) => (id === UNGROUPED ? ungroupedCollapsed.value : !!c);

function clickTask(t: Task) {
  selected.value = t.id;
  revealDate(t.start);
  if (storeState.readOnly) openEditor({ mode: 'edit', taskId: t.id }); // web view: details
}

function menuFor(t: Task, e: MouseEvent) {
  selected.value = t.id;
  const occ = occurrenceOn(t, t.start) ?? { task: t, date: t.start, start: t.start, end: t.end, done: t.done };
  void chipMenu(occ, { x: e.clientX, y: e.clientY });
}

const cancel = () => (editing.value = '');

function splitDown(e: PointerEvent) {
  const x0 = e.clientX;
  const w0 = tableW.value;
  const move = (ev: PointerEvent) => patchSettings({ gantt: { tableWidth: Math.max(140, Math.min(480, w0 + ev.clientX - x0)) } });
  const up = () => {
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', up);
  };
  window.addEventListener('pointermove', move);
  window.addEventListener('pointerup', up);
}
</script>

<template>
  <div class="g-table" role="list" aria-label="任务列表" :style="{ width: `${tableW}px` }">
    <template v-for="r in rows" :key="r.key">
      <div
        v-if="r.kind === 'group'"
        class="t-row group"
        role="listitem"
        @click="toggleCollapse(r.group)"
        @contextmenu.prevent="groupMenu(r.group, { x: $event.clientX, y: $event.clientY })"
      >
        <button
          type="button"
          class="chev"
          :aria-expanded="!collapsed(r.group.id, r.group.collapsed)"
          :aria-label="`${collapsed(r.group.id, r.group.collapsed) ? '展开' : '折叠'}分组 ${r.group.name}`"
          @click.stop="toggleCollapse(r.group)"
        >
          <FIcon :name="collapsed(r.group.id, r.group.collapsed) ? 'chevronRight' : 'chevronDown'" :size="10" />
        </button>
        <span class="dot" :style="{ background: taskColor(r.group.color, isDark).bar }" />
        <input
          v-if="editing === `group:${r.group.id}`"
          v-model="editText"
          class="rename"
          maxlength="80"
          @click.stop
          @keydown.enter.prevent="commitRename"
          @keydown.esc.prevent="cancel"
          @blur="commitRename"
        />
        <span v-else class="name" @dblclick.stop="r.group.id !== UNGROUPED && startRename('group', r.group.id)">{{ r.group.name }}</span>
        <span class="meta">{{ r.tasks.length }}</span>
        <span v-if="showPct && r.start" class="pct">{{ r.progress }}%</span>
      </div>
      <div
        v-else-if="r.kind === 'task'"
        class="t-row task"
        role="listitem"
        :class="{ sel: selected === r.task.id, done: r.task.done && !r.task.repeat }"
        :data-task="r.task.id"
        @click="clickTask(r.task)"
        @dblclick="openEditor({ mode: 'edit', taskId: r.task.id })"
        @contextmenu.prevent="menuFor(r.task, $event)"
      >
        <button
          v-if="!r.task.repeat"
          class="check"
          type="button"
          role="checkbox"
          :aria-checked="r.task.done"
          :aria-label="`完成 ${r.task.title}`"
          :style="{ '--bar': taskColor(r.task.color, isDark).bar }"
          @click.stop="!storeState.readOnly && setOccurrenceDone(r.task, r.task.start, !r.task.done)"
          @dblclick.stop
        >
          <FIcon v-if="r.task.done" name="checkMark" :size="9" />
        </button>
        <FIcon v-else class="rep" name="repeat" :size="11" />
        <FIcon v-if="r.task.milestone" class="ms" name="milestone" :size="10" />
        <input
          v-if="editing === `task:${r.task.id}`"
          v-model="editText"
          class="rename"
          maxlength="200"
          @click.stop
          @dblclick.stop
          @keydown.enter.prevent="commitRename"
          @keydown.esc.prevent="cancel"
          @blur="commitRename"
        />
        <span v-else class="name" @dblclick.stop="startRename('task', r.task.id)">{{ r.task.title }}</span>
        <span v-if="showDates" class="meta" :title="`${diffDays(r.task.start, r.task.end) + 1} 天`">{{ dates(r.task) }}</span>
        <span v-if="showPct && !r.task.milestone && !r.task.repeat" class="pct">{{ r.task.done ? 100 : r.task.progress }}%</span>
      </div>
      <div v-else class="t-row add" role="listitem">
        <button v-if="!storeState.readOnly" type="button" @click="addInGroup(r.groupId)"><FIcon name="add" :size="11" /> 添加任务</button>
      </div>
    </template>
    <div class="g-split" aria-hidden="true" title="拖动调整宽度" @pointerdown.prevent="splitDown" />
  </div>
</template>
