<script setup lang="ts">
import { t as tr } from '@/i18n';
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { FButton, FIcon, taskColor } from '@/fluent';
import { addDays, formatRange, weekday, WEEKDAY_SHORT } from '@/lib/date';
import { displayInfo as dayInfo } from '@/lib/lunar';
import { agenda, allTasks, dayHeading } from '@/lib/agenda';
import { describeRule, type Occurrence } from '@/lib/recur';
import { doc, taskList } from '@/stores/doc';
import { now } from '@/composables/now';
import { isDark } from '@/composables/theme';
import { openEditor } from '../desk/state';

/**
 * The web view's list (WEB-03): 近期 is today and the next days, by day; 所有 every task once
 * on its start date, past ones too (GitHub #4), opening on today.
 */
const props = withDefaults(defineProps<{ range?: 'upcoming' | 'all' }>(), { range: 'upcoming' });
const span = ref(30);
const all = computed(() => props.range === 'all');
const days = computed(() =>
  all.value
    ? allTasks(taskList.value, now.value, doc.settings.tasks.showCompleted)
    : agenda(taskList.value, now.value, addDays(now.value, span.value - 1), now.value, doc.settings.tasks.showCompleted),
);

function sub(date: string): string {
  const i = dayInfo(date);
  const extra = i.legal ? `${i.legal.name}${i.legal.work ? tr("·班") : tr("·休")}` : i.festivals[0] ?? i.term ?? '';
  return [tr("周{0}", tr(WEEKDAY_SHORT[weekday(date)])), i.lunarLabel, extra].filter(Boolean).join(' · ');
}

const dot = (o: Occurrence) => taskColor(o.task.color, isDark.value).bar;
const range = (o: Occurrence) => (o.start !== o.end ? formatRange(o.start, o.end) : '');
const rule = (o: Occurrence) => (all.value && o.task.repeat ? describeRule(o.task.repeat, o.task.start) : '');
const group = (o: Occurrence) => (o.task.groupId ? doc.groups[o.task.groupId]?.name ?? '' : '');
const meta = (o: Occurrence) => [range(o), rule(o), group(o)].filter(Boolean).join(' · ');
const open = (o: Occurrence) => openEditor({ mode: 'edit', taskId: o.task.id, date: o.date });

// 所有 opens on today, with what came before above it.
const list = ref<HTMLElement>();
async function showToday() {
  await nextTick();
  const el = list.value;
  const day = el?.querySelector<HTMLElement>('.day.today');
  if (el && day) el.scrollTop = day.offsetTop - el.offsetTop - 4;
}
onMounted(showToday);
watch(() => props.range, showToday);
defineExpose({ showToday });
</script>

<template>
  <div ref="list" class="agenda">
    <section v-for="d in days" :key="d.date" class="day" :class="{ today: d.date === now }">
      <h2>
        <span class="when">{{ dayHeading(d.date, now) }}</span>
        <span class="sub">{{ sub(d.date) }}</span>
      </h2>
      <p v-if="!d.items.length" class="empty">{{ tr("没有任务") }}</p>
      <button v-for="o in d.items" :key="`${o.task.id}@${o.date}`" type="button" class="item" :class="{ done: o.done }" @click="open(o)">
        <span class="time">{{ o.task.time ?? tr("全天") }}</span>
        <span class="dot" :style="{ background: dot(o) }" aria-hidden="true" />
        <span class="text">
          <span class="title">{{ o.task.title }}</span>
          <span v-if="meta(o)" class="meta">{{ meta(o) }}</span>
        </span>
        <FIcon v-if="o.task.repeat" name="repeat" :size="12" class="rep" :title="tr('重复')" />
      </button>
    </section>
    <div v-if="!all" class="more">
      <FButton v-if="span < 180" @click="span += 30">{{ tr("显示更多日期") }}</FButton>
      <span v-else class="note">{{ tr("只显示 180 天内的任务") }}</span>
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
