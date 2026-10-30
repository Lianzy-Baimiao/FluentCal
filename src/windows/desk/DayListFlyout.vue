<script setup lang="ts">
import { computed, ref } from 'vue';
import { FButton, FFlyout } from '@/fluent';
import type { ISODate } from '@/types';
import { formatFriendly } from '@/lib/date';
import { dayInfo } from '@/lib/lunar';
import { occurrences } from '@/lib/recur';
import type { Chip } from '@/lib/layout';
import { addTask, setOccurrenceDone } from '@/lib/tasks';
import { doc, storeState, taskList } from '@/stores/doc';
import { openEditor } from './state';
import { chipMenu } from './menus';
import TaskChip from './TaskChip.vue';

const props = defineProps<{ date: ISODate; anchor: HTMLElement | null }>();
const open = defineModel<boolean>('open', { default: false });

const chips = computed<Chip[]>(() => {
  if (!props.date) return [];
  const list = taskList.value.flatMap((t) => occurrences(t, props.date, props.date));
  const shown = doc.settings.tasks.showCompleted ? list : list.filter((o) => !o.done);
  return shown
    .sort((a, b) => Number(a.done) - Number(b.done) || (a.task.time ?? '').localeCompare(b.task.time ?? '') || a.task.order - b.task.order)
    .map((occ) => ({
      key: `${occ.task.id}@${occ.date}`,
      occ,
      col: 0,
      span: 1,
      lane: 0,
      contLeft: false,
      contRight: false,
      multiDay: false,
      visible: true,
    }));
});

const info = computed(() => (props.date ? dayInfo(props.date) : null));
const text = ref('');
async function add() {
  const title = text.value.trim();
  if (!title) return;
  text.value = '';
  await addTask({ title, start: props.date });
}
</script>

<template>
  <FFlyout v-model:open="open" :anchor="anchor" placement="bottom-start" :width="280">
    <div class="day-list" style="--text-glow: none">
      <div class="hd">
        <span class="title">{{ date ? formatFriendly(date) : '' }}</span>
        <span v-if="info" class="sub">农历{{ info.lunarLabel }}</span>
      </div>
      <div class="items">
        <div v-for="c in chips" :key="c.key" class="item">
          <TaskChip
            :chip="c"
            @toggle="!storeState.readOnly && setOccurrenceDone(c.occ.task, c.occ.date, !c.occ.done)"
            @open="openEditor({ mode: 'edit', taskId: c.occ.task.id, date: c.occ.date })"
            @menu="(e) => chipMenu(c.occ, { x: e.clientX, y: e.clientY })"
            @key="(e) => e.key === 'Enter' && openEditor({ mode: 'edit', taskId: c.occ.task.id, date: c.occ.date })"
          />
        </div>
        <p v-if="!chips.length" class="empty">这一天还没有任务</p>
      </div>
      <div v-if="!storeState.readOnly" class="add">
        <input v-model="text" placeholder="添加任务，回车保存" maxlength="200" aria-label="添加任务" @keydown.enter.prevent="add" />
        <FButton variant="subtle" size="icon" icon="add" title="添加" @click="add" />
      </div>
    </div>
  </FFlyout>
</template>

<style scoped>
.day-list {
  --fs: 1;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.hd {
  display: flex;
  align-items: baseline;
  gap: 8px;
}
.title {
  font-weight: 600;
}
.sub {
  font-size: 12px;
  color: var(--text-secondary);
}
.items {
  max-height: 260px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.item {
  height: 26px;
}
.item :deep(.chip) {
  font-size: 13px;
}
.empty {
  margin: 4px 0;
  font-size: 12px;
  color: var(--text-tertiary);
}
.add {
  display: flex;
  gap: 4px;
}
.add input {
  flex: 1;
  min-width: 0;
  height: 30px;
  box-sizing: border-box;
  padding: 0 8px;
  font: inherit;
  font-size: 13px;
  color: var(--text-primary);
  background: var(--control-fill);
  border: 1px solid var(--control-stroke);
  border-bottom-color: var(--control-strong-stroke);
  border-radius: 4px;
  outline: none;
  user-select: text;
}
.add input:focus {
  background: var(--control-fill-input-active);
  border-bottom: 2px solid var(--accent-fill);
}
</style>
