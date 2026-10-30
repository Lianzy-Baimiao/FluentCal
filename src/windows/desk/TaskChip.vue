<script setup lang="ts">
import { computed } from 'vue';
import { FIcon, taskColor, vTip } from '@/fluent';
import type { Chip } from '@/lib/layout';
import { doc, storeState } from '@/stores/doc';
import { isDark } from '@/composables/theme';
import { formatRange } from '@/lib/date';
import { describeRule } from '@/lib/recur';

const props = defineProps<{ chip: Chip; dragging?: boolean }>();
const emit = defineEmits<{
  toggle: [];
  open: [];
  menu: [e: MouseEvent];
  pointerdown: [e: PointerEvent];
  key: [e: KeyboardEvent];
}>();

const occ = computed(() => props.chip.occ);
const task = computed(() => occ.value.task);
const colors = computed(() => taskColor(task.value.color, isDark.value));

const tip = computed(() => {
  const t = task.value;
  const lines = [t.title, formatRange(occ.value.start, occ.value.end) + (t.time ? ` ${t.time}` : '')];
  const g = t.groupId && doc.groups[t.groupId];
  if (g) lines.push(`分组：${g.name}`);
  if (t.repeat) lines.push(`重复：${describeRule(t.repeat, t.start)}`);
  if (!t.repeat && t.progress > 0 && !t.done) lines.push(`进度：${t.progress}%`);
  if (t.notes) lines.push(t.notes.length > 120 ? `${t.notes.slice(0, 120)}…` : t.notes);
  return { text: lines.join('\n'), placement: 'top' as const };
});

const style = computed(() =>
  props.chip.multiDay
    ? { '--bar': colors.value.bar, '--on-bar': colors.value.onBar }
    : task.value.color
      ? { '--chip-text': colors.value.text, '--bar': colors.value.bar, '--on-bar': colors.value.onBar }
      : { '--bar': 'var(--accent-fill)', '--on-bar': 'var(--text-on-accent)' },
);
</script>

<template>
  <div
    class="chip"
    :class="{
      bar: chip.multiDay,
      done: occ.done,
      'cont-l': chip.contLeft,
      'cont-r': chip.contRight,
      dragging,
      milestone: task.milestone,
    }"
    :style="style"
    tabindex="0"
    role="group"
    aria-roledescription="任务"
    :aria-label="`${task.title}${occ.done ? '，已完成' : ''}`"
    v-tip="tip"
    @click="storeState.readOnly && emit('open')"
    @dblclick.stop="emit('open')"
    @contextmenu.prevent.stop="emit('menu', $event)"
    @pointerdown="emit('pointerdown', $event)"
    @keydown="emit('key', $event)"
  >
    <button
      class="check"
      type="button"
      role="checkbox"
      :aria-checked="occ.done"
      tabindex="-1"
      :aria-label="occ.done ? '标记为未完成' : '标记为完成'"
      @click.stop="!storeState.readOnly && emit('toggle')"
      @dblclick.stop
      @pointerdown.stop
    >
      <svg v-if="occ.done" viewBox="0 0 12 12" aria-hidden="true"><path d="M2.6 6.2 5 8.5l4.4-4.9" /></svg>
    </button>
    <span v-if="task.time && !chip.contLeft" class="time">{{ task.time }}</span>
    <span class="title">{{ task.title }}</span>
    <FIcon v-if="task.repeat" name="repeat" class="rep" :size="10" />
  </div>
</template>

<style scoped>
.chip {
  height: 100%;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0 4px 0 3px;
  border-radius: 4px;
  font-size: calc(12px * var(--fs));
  line-height: 1;
  color: var(--chip-text, var(--text-primary));
  text-shadow: var(--text-glow);
  cursor: default;
  overflow: hidden;
  white-space: nowrap;
  outline-offset: -1px;
  transition: background-color var(--dur-fast) var(--ease);
}
.chip:hover {
  background: var(--subtle-fill-hover);
}
.chip.bar {
  background: var(--bar);
  color: var(--on-bar);
  text-shadow: none;
  padding-left: 4px;
}
.chip.bar:hover {
  background: color-mix(in srgb, var(--bar) 88%, var(--text-primary));
}
.chip.cont-l {
  border-top-left-radius: 0;
  border-bottom-left-radius: 0;
}
.chip.cont-r {
  border-top-right-radius: 0;
  border-bottom-right-radius: 0;
}
.chip.dragging {
  opacity: 0.35;
}
.title {
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
}
.time {
  font-variant-numeric: tabular-nums;
  opacity: 0.8;
  flex: none;
}
.rep {
  font-size: calc(9px * var(--fs));
  opacity: 0.7;
  flex: none;
}
.done .title {
  text-decoration: line-through;
  opacity: 0.6;
}
.chip.bar.done {
  background: color-mix(in srgb, var(--bar) 32%, transparent);
  color: var(--text-primary);
}
.bar.done .title {
  opacity: 0.7;
}
.check {
  flex: none;
  width: calc(12px * var(--fs));
  height: calc(12px * var(--fs));
  margin: 0;
  padding: 0;
  border-radius: 50%;
  border: 1px solid var(--bar, var(--control-strong-stroke));
  background: transparent;
  display: grid;
  place-items: center;
  cursor: pointer;
}
.chip:not(.bar) .check {
  border-color: color-mix(in srgb, var(--bar) 70%, var(--text-secondary));
}
.bar .check {
  border-color: var(--on-bar);
}
.done .check {
  background: var(--bar);
  border-color: var(--bar);
}
.check svg {
  width: 100%;
  height: 100%;
  fill: none;
  stroke: var(--on-bar);
  stroke-width: 1.6;
}
.check:hover {
  background: color-mix(in srgb, var(--bar) 25%, transparent);
}
.milestone .title::before {
  content: '◆ ';
  color: var(--bar);
}
.bar.milestone .title::before {
  color: inherit;
}
</style>
