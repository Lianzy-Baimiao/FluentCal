<script setup lang="ts">
import { computed, ref } from 'vue';
import { FButton, FSegmented, vTip } from '@/fluent';
import { formatYMD, weekday, WEEKDAY_SHORT } from '@/lib/date';
import { dayInfo } from '@/lib/lunar';
import { doc, patchSettings } from '@/stores/doc';
import { now } from '@/composables/now';
import type { CalendarView } from '@/types';
import { goToday, openSettings, page, ui, viewTitle } from './state';
import { moreMenu } from './menus';
import YearMonthPicker from './YearMonthPicker.vue';
import { syncNow, syncStatus, watchSync } from '@/composables/sync';
import { statusLine } from '@/lib/syncText';

watchSync();

const view = computed(() => doc.settings.calendar.view);
const locked = computed(() => doc.settings.desk.locked);

const title = computed(viewTitle);

const todayTip = computed(() => {
  const i = dayInfo(now.value);
  return `${formatYMD(now.value)} 周${WEEKDAY_SHORT[weekday(now.value)]}\n农历${i.lunarLabel} · ${i.ganzhiYear}${i.festivals.length ? `\n${i.festivals.join('、')}` : ''}`;
});

const views = [
  { value: 'month', label: '月', title: '月视图' },
  { value: 'weeks', label: '多周', title: '多周视图（滚动显示最近几周）' },
  { value: 'gantt', label: '甘特', title: '甘特图' },
];
const viewModel = computed({
  get: () => view.value,
  set: (v: string) => patchSettings({ calendar: { view: v as CalendarView } }),
});

const titleBtn = ref<HTMLElement>();
const pickerOpen = ref(false);

</script>

<template>
  <header class="titlebar" :class="{ movable: !locked }">
    <div class="left">
      <button
        v-if="view === 'month'"
        ref="titleBtn"
        class="title"
        type="button"
        aria-haspopup="dialog"
        :aria-expanded="pickerOpen"
        v-tip="'选择年月'"
        @click="pickerOpen = !pickerOpen"
      >
        {{ title }}<span class="f-icon caret" aria-hidden="true">&#xE70D;</span>
      </button>
      <span v-else class="title static">{{ title }}</span>
      <template v-if="view !== 'gantt'">
        <FButton variant="subtle" size="icon" icon="chevronLeft" :title="view === 'weeks' ? '上一周' : '上个月'" @click="page(-1)" />
        <FButton variant="subtle" size="icon" icon="chevronRight" :title="view === 'weeks' ? '下一周' : '下个月'" @click="page(1)" />
        <FButton class="today-btn" variant="subtle" size="small" v-tip="todayTip" @click="goToday">今天</FButton>
      </template>
    </div>
    <div class="right">
      <FSegmented v-model="viewModel" class="f-seg" :options="views" size="small" />
      <FButton variant="subtle" size="icon" icon="search" title="搜索 (Ctrl+F)" @click="ui.searchOpen = !ui.searchOpen" />
      <FButton
        v-if="syncStatus.enabled"
        class="sync-btn"
        :class="{ spinning: syncStatus.syncing, failed: !!syncStatus.error }"
        variant="subtle"
        size="icon"
        :icon="syncStatus.error ? 'warning' : 'sync'"
        :title="`${statusLine(syncStatus)}（点击立即同步）`"
        @click="syncNow"
      />
      <FButton
        variant="subtle"
        size="icon"
        :icon="locked ? 'lock' : 'unlock'"
        :class="{ unlocked: !locked }"
        :title="locked ? '解锁以调整位置和大小' : '完成调整并锁定'"
        @click="patchSettings({ desk: { locked: !locked } })"
      />
      <FButton class="settings-btn" variant="subtle" size="icon" icon="settings" title="设置" @click="openSettings()" />
      <FButton variant="subtle" size="icon" icon="more" title="更多" @click="(e: MouseEvent) => moreMenu(e.currentTarget as HTMLElement)" />
    </div>
    <YearMonthPicker v-model:open="pickerOpen" :anchor="titleBtn ?? null" />
  </header>
</template>

<style scoped>
.titlebar {
  container-type: inline-size;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  height: 36px;
  flex: none;
  padding: 0 2px 0 4px;
  text-shadow: var(--text-glow);
}
.titlebar.movable {
  cursor: move;
}
.left,
.right {
  display: flex;
  align-items: center;
  gap: 2px;
  min-width: 0;
}
.title {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 32px;
  padding: 0 8px;
  margin-right: 4px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-primary);
  font: inherit;
  font-family: var(--font-display);
  font-size: calc(18px * var(--fs));
  font-weight: 600;
  white-space: nowrap;
  text-shadow: inherit;
  cursor: pointer;
}
.title:hover {
  background: var(--subtle-fill-hover);
}
.title.static {
  cursor: inherit;
}
.title.static:hover {
  background: transparent;
}
.caret {
  font-size: 10px;
  opacity: 0.7;
}
.right :deep(.f-seg) {
  margin-right: 4px;
}
.unlocked {
  color: var(--accent-text);
}
.sync-btn.failed {
  color: var(--caution);
}
.sync-btn.spinning :deep(.f-icon) {
  animation: sync-spin 1s linear infinite;
}
@keyframes sync-spin {
  to {
    transform: rotate(360deg);
  }
}
@container (max-width: 520px) {
  .today-btn,
  .settings-btn,
  .sync-btn {
    display: none;
  }
}
@container (max-width: 430px) {
  .right :deep(.f-seg) {
    display: none;
  }
}
</style>
