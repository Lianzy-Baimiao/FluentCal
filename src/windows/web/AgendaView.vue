<script setup lang="ts">
import { t as tr } from '@/i18n';
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { FButton, FIcon, taskColor } from '@/fluent';
import { addDays, formatFriendly, formatRange, formatYMD, weekday, WEEKDAY_SHORT } from '@/lib/date';
import { displayInfo as dayInfo } from '@/lib/lunar';
import { agenda, allTasks, listDayHeading } from '@/lib/agenda';
import { flexSide, SOMEDAY_SLOT } from '@/lib/flex';
import { describeRule, type Occurrence } from '@/lib/recur';
import { doc } from '@/stores/doc';
import { now } from '@/composables/now';
import { isDark } from '@/composables/theme';
import { openEditor, shownTasks, ui } from '../desk/state';

/**
 * The web view's list (WEB-03): 近期 is today and the next days, by day; 所有 every task once
 * on its start date, past ones too (GitHub #4), opening on today.
 */
const props = withDefaults(defineProps<{ range?: 'upcoming' | 'all' }>(), { range: 'upcoming' });
const span = ref(30);
const all = computed(() => props.range === 'all' || !!ui.revealTask);
const days = computed(() =>
  all.value
    ? allTasks(shownTasks.value, now.value, doc.settings.tasks.showCompleted || !!ui.revealTask)
    : agenda(shownTasks.value, now.value, addDays(now.value, span.value - 1), now.value, doc.settings.tasks.showCompleted),
);
const flex = computed(() => all.value ? flexSide(shownTasks.value, now.value) : undefined);
const monthSlot = computed(() => flex.value?.monthSlot ?? '');
const flexMonth = computed(() => (flex.value?.month ?? []).filter(({ done }) => doc.settings.tasks.showCompleted || !done));
const flexSomeday = computed(() => flex.value?.someday ?? []);

function sub(date: string): string {
  const i = dayInfo(date);
  const extra = i.legal ? `${i.legal.name}${i.legal.work ? tr("·班") : tr("·休")}` : i.festivals[0] ?? i.term ?? '';
  return [tr("周{0}", tr(WEEKDAY_SHORT[weekday(date)])), i.lunarLabel, extra].filter(Boolean).join(' · ');
}

const dot = (o: Occurrence) => taskColor(o.task.color, isDark.value).bar;
const range = (o: Occurrence) => (o.start !== o.end ? formatRange(o.start, o.end) : '');
const rule = (o: Occurrence) => (all.value && o.task.repeat ? describeRule(o.task.repeat, o.task.start) : '');
const group = (o: Occurrence) => (o.task.groupId ? doc.groups[o.task.groupId]?.name ?? '' : '');
const groupColor = (o: Occurrence) => doc.groups[o.task.groupId ?? '']?.color ?? doc.settings.calendar.ungroupedColor;
const taskGroup = (task: Occurrence['task']) => task.groupId ? doc.groups[task.groupId]?.name ?? '' : '';
const taskGroupColor = (task: Occurrence['task']) => doc.groups[task.groupId ?? '']?.color ?? doc.settings.calendar.ungroupedColor;
const meta = (o: Occurrence) => [range(o), rule(o), doc.settings.calendar.listGroupLabels || !o.task.groupId ? '' : group(o)].filter(Boolean).join(' · ');
const open = (o: Occurrence) => openEditor({ mode: 'edit', taskId: o.task.id, date: o.date });
const visibleItems = (items: Occurrence[]) => items.filter(o => doc.settings.tasks.showCompleted || !o.done || o.task.id === ui.revealTask);
const openFlex = (taskId: string, slot: string) => openEditor({ mode: 'edit', taskId, date: slot });

// 所有 opens on today, with what came before above it.
const list = ref<HTMLElement>();
const scrollTop = () => { if (list.value) list.value.scrollTop = 0; };
onMounted(() => document.addEventListener('fluentcal-list-range', scrollTop));
watch(() => ui.locateRevision, scrollTop);
async function showToday() {
  await nextTick();
  const el = list.value;
  if (el && (props.range === 'all' || ui.revealTask)) {
    el.scrollTop = 0;
    return;
  }
  const day = el?.querySelector<HTMLElement>('.day.today');
  if (el && day) el.scrollTop = day.offsetTop - el.offsetTop - 4;
}
onMounted(showToday);
watch(() => props.range, showToday);
watch(() => ui.locateRevision, async () => {
  const id = ui.flashTask;
  if (!id) return;
  await nextTick();
  const candidates = [...(list.value?.querySelectorAll<HTMLElement>('[data-task]') ?? [])].filter(el => el.dataset.task === id);
  const target = candidates.find(el => el.dataset.date === ui.flash) ?? candidates[0];
  target?.scrollIntoView({ block: 'center', behavior: 'instant' });
});
defineExpose({ showToday });
</script>

<template>
  <div ref="list" class="agenda">
    <div v-if="all" class="flex-sections">
      <section class="flex-group" data-flex-section="month">
        <h2>{{ tr("本 {0} 月待办（不定日）", Number(now.slice(5, 7))) }}</h2>
        <p class="flex-note">{{ tr("在桌面勾选时按今天记录：{0}", formatYMD(now)) }}</p>
        <button v-for="entry in flexMonth" :key="`${entry.task.id}@${monthSlot}`" type="button" class="item" :data-task="entry.task.id" :data-date="monthSlot" :class="{ done: !!entry.done, located: ui.flashTask === entry.task.id && ui.flash === now }" @click="openFlex(entry.task.id, monthSlot)">
          <span class="time">{{ tr("不定日") }}</span>
          <span class="dot" :style="{ background: taskColor(entry.task.color, isDark).bar }" aria-hidden="true" />
          <span class="text"><span class="title-line"><span v-if="doc.settings.calendar.listGroupLabels && taskGroup(entry.task)" class="group-badge" :title="taskGroup(entry.task)" :style="{ background: taskColor(taskGroupColor(entry.task), isDark).bar, color: taskColor(taskGroupColor(entry.task), isDark).onBar }">{{ taskGroup(entry.task) }}</span><span class="title">{{ entry.task.title }}</span></span><span v-if="entry.done" class="meta">{{ tr("完成于 {0}", formatFriendly(entry.done, now)) }}</span></span>
        </button>
        <p v-if="!flexMonth.length" class="empty">{{ tr("没有任务") }}</p>
      </section>
      <section class="flex-group" data-flex-section="someday">
        <h2>{{ tr("无日期待办") }}</h2>
        <p class="flex-note">{{ tr("在桌面勾选时按今天记录：{0}", formatYMD(now)) }}</p>
        <button v-for="task in flexSomeday" :key="task.id" type="button" class="item" :data-task="task.id" :data-date="SOMEDAY_SLOT" :class="{ located: ui.flashTask === task.id }" @click="openFlex(task.id, SOMEDAY_SLOT)">
          <span class="time">{{ tr("无日期") }}</span>
          <span class="dot" :style="{ background: taskColor(task.color, isDark).bar }" aria-hidden="true" />
          <span class="text"><span class="title-line"><span v-if="doc.settings.calendar.listGroupLabels && taskGroup(task)" class="group-badge" :title="taskGroup(task)" :style="{ background: taskColor(taskGroupColor(task), isDark).bar, color: taskColor(taskGroupColor(task), isDark).onBar }">{{ taskGroup(task) }}</span><span class="title">{{ task.title }}</span></span></span>
        </button>
        <p v-if="!flexSomeday.length" class="empty">{{ tr("没有任务") }}</p>
      </section>
    </div>
    <section v-for="d in days" :key="d.date" class="day" :class="{ today: d.date === now }">
      <h2>
        <span class="when">{{ listDayHeading(d.date, now) }}</span>
        <span class="sub">{{ sub(d.date) }}</span>
      </h2>
      <p v-if="!d.items.length" class="empty">{{ tr("没有任务") }}</p>
      <button v-for="o in visibleItems(d.items)" :key="`${o.task.id}@${o.date}`" type="button" class="item" :data-task="o.task.id" :data-date="o.date" :class="{ done: o.done, located: ui.flashTask === o.task.id && ui.flash === o.date }" @click="open(o)">
        <span class="time">{{ o.task.time ?? tr("全天") }}</span>
        <span class="dot" :style="{ background: dot(o) }" aria-hidden="true" />
        <span class="text">
          <span class="title-line"><span v-if="doc.settings.calendar.listGroupLabels && group(o)" class="group-badge" :title="group(o)" :style="{ background: taskColor(groupColor(o), isDark).bar, color: taskColor(groupColor(o), isDark).onBar }">{{ group(o) }}</span><span class="title">{{ o.task.title }}</span></span>
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
.flex-sections { margin-top: 12px; border-bottom: 1px solid var(--divider); padding-bottom: 10px; }
.flex-group + .flex-group { margin-top: 10px; }
.flex-group h2 { margin-bottom: 4px; }
.flex-note { margin: 0 8px 4px; font-size: 12px; color: var(--text-secondary); }
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
.item.located { outline: 2px solid var(--accent-fill); outline-offset: -2px; background: color-mix(in srgb, var(--accent-fill) 16%, var(--cell-bg)); }
.group-badge { display: inline-block; max-width: min(120px, 25vw); border-radius: 4px; padding: 2px 6px; font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.title-line { display: flex; align-items: center; gap: 6px; min-width: 0; }
.title-line .title { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
.title-line .group-badge { flex: none; }
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
