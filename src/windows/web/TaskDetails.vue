<script setup lang="ts">
import { computed } from 'vue';
import { FDialog, taskColor } from '@/fluent';
import { formatRange } from '@/lib/date';
import { describeRule, occurrenceOn } from '@/lib/recur';
import { doc } from '@/stores/doc';
import { isDark } from '@/composables/theme';
import { details } from '../desk/state';

/** Read-only details of a task in the web view (WEB-03). */
const task = computed(() => doc.tasks[details.taskId]);
const occ = computed(() => (task.value && details.date ? occurrenceOn(task.value, details.date) : undefined));
const when = computed(() => {
  const t = task.value;
  if (!t) return '';
  const r = formatRange(occ.value?.start ?? t.start, occ.value?.end ?? t.end);
  return t.time ? `${r} ${t.time}` : r;
});
const done = computed(() => occ.value?.done ?? task.value?.done ?? false);
const group = computed(() => (task.value?.groupId ? doc.groups[task.value.groupId]?.name : ''));
const deps = computed(() => (task.value?.deps ?? []).map((id) => doc.tasks[id]?.title).filter(Boolean).join('、'));
const color = computed(() => taskColor(task.value?.color, isDark.value).bar);
</script>

<template>
  <FDialog :open="details.open && !!task" :title="task?.title ?? ''" close-text="关闭" :width="420" @update:open="details.open = $event">
    <dl v-if="task" class="facts">
      <dt>时间</dt>
      <dd><span class="dot" :style="{ background: color }" aria-hidden="true" />{{ when }}{{ done ? '（已完成）' : '' }}</dd>
      <template v-if="task.repeat">
        <dt>重复</dt>
        <dd>{{ describeRule(task.repeat, task.start) }}</dd>
      </template>
      <template v-if="group">
        <dt>分组</dt>
        <dd>{{ group }}</dd>
      </template>
      <template v-if="!task.repeat && !task.milestone && task.progress > 0">
        <dt>进度</dt>
        <dd>{{ task.done ? 100 : task.progress }}%</dd>
      </template>
      <template v-if="deps">
        <dt>前置任务</dt>
        <dd>{{ deps }}</dd>
      </template>
      <template v-if="task.notes">
        <dt>备注</dt>
        <dd class="notes">{{ task.notes }}</dd>
      </template>
    </dl>
    <p class="hint">网页中只能查看。要修改，请在电脑上打开 FluentCal。</p>
  </FDialog>
</template>

<style scoped>
.facts {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 8px 16px;
  margin: 0;
}
dt {
  color: var(--text-secondary);
  white-space: nowrap;
}
dd {
  margin: 0;
  overflow-wrap: anywhere;
}
.notes {
  white-space: pre-wrap;
}
.dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  margin-right: 6px;
  border-radius: 50%;
  vertical-align: 1px;
}
.hint {
  margin: 16px 0 0;
  font-size: 12px;
  color: var(--text-secondary);
}
</style>
