<script setup lang="ts">
import { computed } from 'vue';
import { FIcon, FSelect, taskColor } from '@/fluent';
import { doc, taskList } from '@/stores/doc';
import { isDark } from '@/composables/theme';
import { formatRange } from '@/lib/date';

const props = defineProps<{ self: string; groupId?: string }>();
const deps = defineModel<string[] | undefined>();

/** Tasks that (transitively) depend on `self`; picking one would create a cycle. */
const successors = computed(() => {
  const out = new Set<string>([props.self]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const t of taskList.value) {
      if (!out.has(t.id) && t.deps?.some((d) => out.has(d))) {
        out.add(t.id);
        grew = true;
      }
    }
  }
  return out;
});

const options = computed(() =>
  taskList.value
    .filter((t) => !t.repeat && !successors.value.has(t.id) && !deps.value?.includes(t.id))
    .sort((a, b) => Number(b.groupId === props.groupId) - Number(a.groupId === props.groupId) || (a.start < b.start ? -1 : 1))
    .slice(0, 200)
    .map((t) => ({ value: t.id, label: `${t.title}（${formatRange(t.start, t.end)}）` })),
);

function add(id: string | number) {
  if (id) deps.value = [...(deps.value ?? []), String(id)];
}
function remove(id: string) {
  const next = (deps.value ?? []).filter((d) => d !== id);
  deps.value = next.length ? next : undefined;
}
</script>

<template>
  <div class="deps">
    <span v-for="id in deps ?? []" :key="id" class="tag">
      <span class="dot" :style="{ background: taskColor(doc.tasks[id]?.color, isDark).bar }" />
      {{ doc.tasks[id]?.title ?? '（已删除）' }}
      <button type="button" :aria-label="`移除 ${doc.tasks[id]?.title ?? ''}`" @click="remove(id)"><FIcon name="close" :size="9" /></button>
    </span>
    <FSelect :model-value="''" :options="options" placeholder="添加前置任务…" :width="220" :disabled="!options.length" @update:model-value="add" />
  </div>
</template>

<style scoped>
.deps {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}
.tag {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 4px 0 10px;
  border-radius: 14px;
  background: var(--control-fill);
  border: 1px solid var(--control-stroke);
  font-size: 12px;
  max-width: 260px;
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex: none;
}
.tag button {
  width: 20px;
  height: 20px;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--text-secondary);
  display: grid;
  place-items: center;
  cursor: pointer;
}
.tag button:hover {
  background: var(--subtle-fill-hover);
}
</style>
