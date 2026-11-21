<script setup lang="ts">
import { t as tr } from '@/i18n';
import { computed } from 'vue';
import { FIcon, taskColor, vTip } from '@/fluent';
import type { Chip } from '@/lib/layout';
import { doc, storeState } from '@/stores/doc';
import { isDark } from '@/composables/theme';
import { now } from '@/composables/now';
import { barLabel, occDoneDay } from '@/lib/bar';
import { formatFriendly, formatRange } from '@/lib/date';
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

// A bar over several days: which day it is, the progress or the day it was done, after the
// title (设置 › 日历 on the PC has a switch for each), and how much of this piece is done.
const doneOn = computed(() => occDoneDay(occ.value));
const bar = computed(() => {
  const c = doc.settings.calendar;
  if (!props.chip.multiDay) return { text: '', fill: 0 };
  return barLabel(occ.value, props.chip.segStart, props.chip.span, now.value, { days: c.barDays, progress: c.barProgress, done: c.barDone });
});
// The label shows when there is room for all of it or for "第5-2天…" at the least (4.5em):
// about its own width, a full-width character 1em and the others 0.6em.
const labelMin = computed(() => {
  let em = 0;
  for (const ch of bar.value.text) em += ch.charCodeAt(0) > 0x2e7f ? 1 : 0.6;
  return `${Math.min(4.5, em).toFixed(1)}em`;
});

const tip = computed(() => {
  const t = task.value;
  const lines = [t.title, formatRange(occ.value.start, occ.value.end) + (t.time ? ` ${t.time}` : '')];
  const g = t.groupId && doc.groups[t.groupId];
  if (g) lines.push(tr("分组：{0}", g.name));
  if (t.repeat) lines.push(tr("重复：{0}", describeRule(t.repeat, t.start)));
  if (!t.repeat && t.progress > 0 && !t.done) lines.push(tr("进度：{0}%", t.progress));
  if (doneOn.value) lines.push(tr("完成于 {0}", formatFriendly(doneOn.value)));
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
    :aria-roledescription="tr('任务')"
    :aria-label="`${task.title}${occ.done ? tr('，已完成') : ''}`"
    v-tip="tip"
    @click="storeState.readOnly && emit('open')"
    @dblclick.stop="emit('open')"
    @contextmenu.prevent.stop="emit('menu', $event)"
    @pointerdown="emit('pointerdown', $event)"
    @keydown="emit('key', $event)"
  >
    <span v-if="bar.fill > 0" class="fill" :style="{ width: `${bar.fill * 100}%` }" aria-hidden="true" />
    <button
      class="check"
      type="button"
      role="checkbox"
      :aria-checked="occ.done"
      tabindex="-1"
      :aria-label="occ.done ? tr('标记为未完成') : tr('标记为完成')"
      @click.stop="!storeState.readOnly && emit('toggle')"
      @dblclick.stop
      @pointerdown.stop
    >
      <svg v-if="occ.done" viewBox="0 0 12 12" aria-hidden="true"><path d="M2.6 6.2 5 8.5l4.4-4.9" /></svg>
    </button>
    <span v-if="task.time && !chip.contLeft" class="time">{{ task.time }}</span>
    <!-- The title comes first: the label gets the room that is left, is cut short with "…"
         there, and goes (to a second line that is cut off) when that is too little. -->
    <span v-if="bar.text" class="words">
      <span class="title">{{ task.title }}</span>
      <span class="pct" :style="{ flexBasis: labelMin }">{{ bar.text }}</span>
    </span>
    <span v-else class="title">{{ task.title }}</span>
    <FIcon v-if="task.repeat" name="repeat" class="rep" :size="10" />
  </div>
</template>

<style scoped>
.chip {
  position: relative;
  z-index: 0;
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
/* the done part of a bar, darker (as in the Gantt chart); under the text */
.fill {
  position: absolute;
  z-index: -1;
  left: 0;
  top: 0;
  bottom: 0;
  background: #00000038;
  pointer-events: none;
}
/* one line of a wrapping row: what doesn't fit on it is out of sight */
.words {
  flex: 0 1 auto;
  min-width: 0;
  height: 1.3em;
  line-height: 1.3;
  display: flex;
  flex-wrap: wrap;
  align-content: flex-start;
  column-gap: 4px;
  overflow: hidden;
}
.words .title {
  flex: 0 1 auto;
  max-width: 100%;
}
/* (as strong as the title: paler, it falls short of 4.5:1 on some of the bar colours) */
.pct {
  flex: 1 1 4.5em;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
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
