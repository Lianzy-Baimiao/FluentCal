<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { vTip } from '@/fluent';
import { diffDays, formatRange, weekday } from '@/lib/date';
import { chipMenu } from '../menus';
import { now } from '@/composables/now';
import { doc } from '@/stores/doc';
import { bars, bodyH, chartEl, chartW, dw, gs, links, monthLines, range, RH, rows, selected, stripes, summaries, xOfDate, type Bar } from './model';
import { ghost, linkDrag, onAddRowDown, onBarDblClick, onBarDown, onLinkDown } from './drag';

const root = ref<HTMLElement>();
onMounted(() => (chartEl.value = root.value ?? null));
onBeforeUnmount(() => (chartEl.value = null));

const todayX = computed(() => xOfDate(now.value) + dw.value / 2);
// Week separators: one line per week at the configured first weekday.
const weekBg = computed(() => {
  if (gs.value.scale === 'month') return {};
  const period = 7 * dw.value;
  const off = ((((doc.settings.calendar.weekStart - weekday(range.value.start)) % 7) + 7) % 7) * dw.value;
  return { backgroundImage: `repeating-linear-gradient(to right, var(--g-grid) 0 1px, transparent 1px ${period}px)`, backgroundPosition: `${off}px 0` };
});

const charW = (s: string) => [...s].reduce((n, c) => n + (c.charCodeAt(0) > 0x2e80 ? 12 : 6.6), 0);
const inside = (b: Bar) => !b.task.milestone && charW(b.task.title) + 14 <= b.w;
const tip = (b: Bar) => {
  const t = b.task;
  const deps = (t.deps ?? []).map((id) => doc.tasks[id]?.title).filter(Boolean);
  return [
    t.title,
    `${formatRange(b.start, b.end)} · ${diffDays(b.start, b.end) + 1} 天`,
    t.milestone ? '里程碑' : b.rep ? '重复任务（双击编辑整个系列）' : `进度 ${b.progress}%`,
    deps.length ? `前置：${deps.join('、')}` : '',
  ]
    .filter(Boolean)
    .join('\n');
};
const menu = (b: Bar, e: MouseEvent) => {
  selected.value = b.task.id;
  void chipMenu({ task: b.task, date: b.date, start: b.start, end: b.end, done: b.done }, { x: e.clientX, y: e.clientY });
};
</script>

<template>
  <div ref="root" class="g-chart" :style="{ width: `${chartW}px`, height: `${bodyH}px` }" @pointerdown.self="selected = ''">
    <div class="grid-weeks" :style="weekBg" />
    <div v-for="x in monthLines" :key="`m${x}`" class="month-line" :style="{ left: `${x}px` }" />
    <div v-for="s in stripes" :key="s.key" class="stripe" :class="{ holiday: s.holiday }" :style="{ left: `${s.x}px`, width: `${s.w}px` }" />
    <template v-for="(r, i) in rows" :key="r.key">
      <div v-if="r.kind === 'group'" class="row-tint group" :style="{ top: `${i * RH}px` }" />
      <div v-else-if="r.kind === 'task' && selected === r.task.id" class="row-tint sel" :style="{ top: `${i * RH}px` }" />
      <div v-else-if="r.kind === 'add'" class="add-zone" :style="{ top: `${i * RH}px` }" title="拖动以创建任务" @pointerdown="onAddRowDown(r.groupId, i, $event)" />
    </template>
    <div class="row-lines" />
    <div class="today" :style="{ left: `${todayX}px` }" />
    <svg class="links" :width="chartW" :height="bodyH" aria-hidden="true">
      <defs>
        <marker id="g-arrow" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L8,4 L0,8 z" class="arrow" /></marker>
        <marker id="g-arrow-late" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L8,4 L0,8 z" class="arrow late" /></marker>
      </defs>
      <path v-for="l in links" :key="l.key" :d="l.d" :class="{ late: l.late }" :marker-end="l.late ? 'url(#g-arrow-late)' : 'url(#g-arrow)'" />
      <line v-if="linkDrag.show" class="rubber" :class="{ ok: linkDrag.ok }" :x1="linkDrag.x1" :y1="linkDrag.y1" :x2="linkDrag.x2" :y2="linkDrag.y2" />
    </svg>
    <div v-for="s in summaries" :key="s.key" class="summary" :style="{ left: `${s.x}px`, width: `${s.w}px`, top: `${s.row * RH + 11}px`, '--c': s.color, '--p': `${s.progress}%` }" />
    <template v-for="b in bars" :key="b.key">
      <div
        v-if="b.task.milestone"
        class="milestone"
        :data-task="b.task.id"
        :class="{ sel: selected === b.task.id, done: b.done }"
        :style="{ left: `${b.x + dw / 2}px`, top: `${b.row * RH + RH / 2}px`, '--bar': b.bar }"
        v-tip="tip(b)"
        @pointerdown="onBarDown(b, $event)"
        @dblclick="onBarDblClick(b)"
        @contextmenu.prevent="menu(b, $event)"
      >
        <span class="label out" :style="{ color: b.text }">{{ b.task.title }}</span>
        <span class="link" aria-hidden="true" @pointerdown="onLinkDown(b, $event)" />
      </div>
      <div
        v-else
        class="bar"
        :data-task="b.task.id"
        :class="{ sel: selected === b.task.id, done: b.done, rep: b.rep }"
        :style="{ left: `${b.x + 1}px`, width: `${Math.max(4, b.w - 2)}px`, top: `${b.row * RH + 6}px`, '--bar': b.bar, '--on': b.onBar, '--p': `${b.progress}%` }"
        v-tip="tip(b)"
        @pointerdown="onBarDown(b, $event)"
        @dblclick="onBarDblClick(b)"
        @contextmenu.prevent="menu(b, $event)"
      >
        <div class="fill" />
        <span v-if="inside(b)" class="label">{{ b.task.title }}</span>
        <template v-if="!b.rep">
          <span class="handle l" data-handle="start" />
          <span class="handle r" data-handle="end" />
          <span class="handle p" data-handle="progress" :style="{ left: `calc(${b.progress}% - 5px)` }" />
          <span class="link" aria-hidden="true" @pointerdown="onLinkDown(b, $event)" />
        </template>
        <span v-if="!inside(b)" class="label out" :style="{ color: b.text }">{{ b.task.title }}</span>
      </div>
    </template>
    <div v-if="ghost.show" class="ghost" :style="{ left: `${xOfDate(ghost.start)}px`, width: `${(diffDays(ghost.start, ghost.end) + 1) * dw}px`, top: `${ghost.row * RH + 6}px` }" />
  </div>
</template>
