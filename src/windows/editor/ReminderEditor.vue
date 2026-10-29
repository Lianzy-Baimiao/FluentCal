<script setup lang="ts">
import { computed } from 'vue';
import { FButton, FSelect } from '@/fluent';
import { offsetLabel, REMINDER_OFFSETS } from '@/lib/reminders';

const props = defineProps<{ allDay: boolean; allDayTime: string; enabled: boolean }>();
const list = defineModel<number[] | undefined>();

const options = computed(() => {
  const base = REMINDER_OFFSETS.filter((o) => !props.allDay || o.value === 0 || o.value >= 1440);
  const extra = (list.value ?? []).filter((v) => !base.some((o) => o.value === v)).map((v) => ({ value: v, label: offsetLabel(v) }));
  return [...base, ...extra].map((o) => ({ ...o, label: props.allDay && o.value === 0 ? `当天 ${props.allDayTime}` : o.label }));
});

function set(i: number, v: number) {
  const next = [...(list.value ?? [])];
  next[i] = v;
  list.value = [...new Set(next)].sort((a, b) => a - b);
}
function add() {
  const used = new Set(list.value ?? []);
  const v = options.value.find((o) => !used.has(o.value))?.value ?? 0;
  list.value = [...(list.value ?? []), v].sort((a, b) => a - b);
}
function remove(i: number) {
  const next = (list.value ?? []).filter((_, j) => j !== i);
  list.value = next.length ? next : undefined;
}
</script>

<template>
  <div class="reminders">
    <div v-for="(v, i) in list ?? []" :key="i" class="line">
      <FSelect :model-value="v" :options="options" :width="200" @update:model-value="(x) => set(i, Number(x))" />
      <FButton variant="subtle" size="icon" icon="close" :title="'删除提醒'" @click="remove(i)" />
    </div>
    <div class="line">
      <FButton variant="subtle" icon="add" :disabled="(list?.length ?? 0) >= 5" @click="add">添加提醒</FButton>
      <span v-if="!enabled" class="hint">提醒已在设置中关闭</span>
      <span v-else-if="allDay && list?.length" class="hint">全天任务在 {{ allDayTime }} 提醒</span>
    </div>
  </div>
</template>

<style scoped>
.reminders {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.line {
  display: flex;
  align-items: center;
  gap: 6px;
}
.hint {
  font-size: 12px;
  color: var(--text-secondary);
}
</style>
