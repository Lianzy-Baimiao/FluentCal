<script setup lang="ts">
import { computed, ref } from 'vue';
import { FButton, FIcon, taskColor } from '@/fluent';
import { addDays, formatRange, weekday, WEEKDAY_SHORT } from '@/lib/date';
import { dayInfo } from '@/lib/lunar';
import { agenda, dayHeading } from '@/lib/agenda';
import type { Occurrence } from '@/lib/recur';
import { doc, taskList } from '@/stores/doc';
import { now } from '@/composables/now';
import { isDark } from '@/composables/theme';
import { openEditor } from '../desk/state';

/** The web view's list for phones (WEB-03): today and the next days, by day. */
const span = ref(30);
const days = computed(() => agenda(taskList.value, now.value, addDays(now.value, span.value - 1), now.value, doc.settings.tasks.showCompleted));

function sub(date: string): string {
  const i = dayInfo(date);
  const extra = i.legal ? `${i.legal.name}${i.legal.work ? '·班' : '·休'}` : i.festivals[0] ?? i.term ?? '';
  return [`周${WEEKDAY_SHORT[weekday(date)]}`, i.lunarLabel, extra].filter(Boolean).join(' · ');
}

const dot = (o: Occurrence) => taskColor(o.task.color, isDark.value).bar;
const range = (o: Occurrence) => (o.start !== o.end ? formatRange(o.start, o.end) : '');
const group = (o: Occurrence) => (o.task.groupId ? doc.groups[o.task.groupId]?.name ?? '' : '');
const open = (o: Occurrence) => openEditor({ mode: 'edit', taskId: o.task.id, date: o.date });
</script>

<template>
  <div class="agenda">
    <section v-for="d in days" :key="d.date" class="day" :class="{ today: d.date === now }">
      <h2>
        <span class="when">{{ dayHeading(d.date, now) }}</span>
        <span class="sub">{{ sub(d.date) }}</span>
      </h2>
      <p v-if="!d.items.length" class="empty">没有任务</p>
      <button v-for="o in d.items" :key="`${o.task.id}@${o.date}`" type="button" class="item" :class="{ done: o.done }" @click="open(o)">
        <span class="time">{{ o.task.time ?? '全天' }}</span>
        <span class="dot" :style="{ background: dot(o) }" aria-hidden="true" />
        <span class="text">
          <span class="title">{{ o.task.title }}</span>
          <span v-if="range(o) || group(o)" class="meta">{{ [range(o), group(o)].filter(Boolean).join(' · ') }}</span>
        </span>
        <FIcon v-if="o.task.repeat" name="repeat" :size="12" class="rep" title="重复" />
      </button>
    </section>
    <div class="more">
      <FButton v-if="span < 180" @click="span += 30">显示更多日期</FButton>
      <span v-else class="note">只显示 180 天内的任务</span>
    </div>
  </div>
</template>

<style scoped>
.agenda {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0 4px 16px;
  touch-action: pan-y;
}
.day {
  margin-top: 12px;
}
h2 {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin: 0 0 4px;
  padding: 0 8px;
  font-size: 15px;
  font-weight: 600;
}
.today h2 .when {
  color: var(--accent-text);
}
.sub,
.meta,
.note,
.empty {
  font-size: 12px;
  font-weight: 400;
  color: var(--text-secondary);
}
.empty {
  margin: 0;
  padding: 6px 8px;
}
.item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  min-height: 44px;
  padding: 6px 8px;
  border: 0;
  border-radius: 6px;
  background: var(--card-fill);
  color: var(--text-primary);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.item + .item {
  margin-top: 2px;
}
.item:hover {
  background: var(--subtle-fill-hover);
}
.time {
  width: 40px;
  flex: none;
  font-size: 12px;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}
.dot {
  width: 4px;
  align-self: stretch;
  flex: none;
  border-radius: 2px;
}
.text {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
}
.title {
  font-size: 14px;
  overflow-wrap: anywhere;
}
.done .title {
  text-decoration: line-through;
  color: var(--text-secondary);
}
.rep {
  flex: none;
  opacity: 0.7;
}
.more {
  display: flex;
  justify-content: center;
  padding: 16px 0 4px;
}
</style>
