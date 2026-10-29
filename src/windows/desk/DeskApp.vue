<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch, watchEffect } from 'vue';
import { FButton, FInfoBar } from '@/fluent';
import { call, currentWindow, on, type DataDirInfo, type ResizeEdge } from '@/api';
import { doc, history, patchSettings, put, redo, storeState, initStore, taskList, undo, type ReplayResult } from '@/stores/doc';
import { updateHolidays } from '@/stores/holidays';
import { startThemeSync, deskVars } from '@/composables/theme';
import { now, startClock } from '@/composables/now';
import { computeReminders } from '@/lib/reminders';
import { sampleData } from '@/lib/seed';
import { autoPostpone } from '@/lib/tasks';
import { startOfMonth } from '@/lib/date';
import { defaultWeeksStart, goToday, notify, openEditor, page, ui } from './state';
import TitleBar from './TitleBar.vue';
import MonthView from './MonthView.vue';
import SearchPanel from './SearchPanel.vue';
import CountdownBar from './CountdownBar.vue';

const GanttView = defineAsyncComponent(() => import('./gantt/GanttView.vue'));

const view = computed(() => doc.settings.calendar.view);
const desk = computed(() => doc.settings.desk);
const notice = ref('');
const saveError = ref<string | null>(null);

startThemeSync();

// Desk variables live on <html> so teleported flyouts and menus inherit them too.
watchEffect(() => {
  const root = document.documentElement;
  for (const [k, v] of Object.entries(deskVars.value)) root.style.setProperty(k, v);
});

// ---------------------------------------------------------------- reminders

let remTimer: ReturnType<typeof setTimeout> | undefined;
function scheduleReminders() {
  clearTimeout(remTimer);
  remTimer = setTimeout(() => {
    const r = doc.settings.reminders;
    const items = r.enabled ? computeReminders(taskList.value, Date.now(), r.allDayTime) : [];
    void call('reminders_set', { items });
  }, 400);
}

async function seedIfFirstRun() {
  if (doc.settings.general.seeded) return;
  if (!Object.keys(doc.tasks).length && !Object.keys(doc.groups).length) {
    const { groups, tasks } = sampleData(now.value);
    await put('groups', Object.fromEntries(groups.map((g) => [g.id, g])), { history: false });
    await put('tasks', Object.fromEntries(tasks.map((t) => [t.id, t])), { history: false });
  }
  patchSettings({ general: { seeded: true } });
}

function onKey(e: KeyboardEvent) {
  const el = e.target as HTMLElement;
  const typing = el.matches?.('input, textarea, [contenteditable="true"]');
  const ctrl = e.ctrlKey || e.metaKey;
  if (ctrl && e.key.toLowerCase() === 'f') {
    ui.searchOpen = true;
  } else if (ctrl && e.key.toLowerCase() === 'n') {
    openEditor({ mode: 'new', start: ui.selected });
  } else if (typing) {
    return;
  } else if (ctrl && e.key.toLowerCase() === 'z' && !e.shiftKey) {
    void undo().then((r) => notify(replayText(r, '撤销')));
  } else if (ctrl && (e.key.toLowerCase() === 'y' || (e.shiftKey && e.key.toLowerCase() === 'z'))) {
    void redo().then((r) => notify(replayText(r, '重做')));
  } else if (e.key === 'Escape' && ui.searchOpen) {
    ui.searchOpen = false;
  } else if (e.key === 'Home' && view.value !== 'gantt') {
    goToday();
  } else if ((e.key === 'PageUp' || e.key === 'PageDown') && view.value !== 'gantt') {
    page(e.key === 'PageUp' ? -1 : 1);
  } else return;
  e.preventDefault();
}

function replayText(r: ReplayResult, verb: string): string {
  if (!r.done) return `没有可${verb}的操作`;
  return r.skipped ? `已${verb}（${r.skipped} 项已在其他窗口修改，保持不变）` : `已${verb}`;
}

/**
 * On the desktop layer the window is not activated by clicks (activating any window ends
 * Windows' "Show desktop" state, so the next Win+D would not restore the windows). Ask Rust
 * for the keyboard only when a text box is used; everything else works with the mouse.
 * Keyboard shortcuts are available after the hotkey (Ctrl+Alt+D) brings the calendar up.
 */
const EDITABLE = 'input, textarea, [contenteditable="true"]';
function wantsKeyboard(e: Event) {
  if ((e.target as HTMLElement | null)?.closest?.(EDITABLE)) void call('desk_activate');
}

/** Adjust mode (unlocked): press anywhere except a control to move the window; the calendar
 * itself ignores the mouse meanwhile (see .unlocked styles). */
function onAdjustDown(e: PointerEvent) {
  if (desk.value.locked || e.button !== 0) return;
  if ((e.target as HTMLElement).closest('button, input, a, [role="radio"], [role="tab"], .grip')) return;
  e.preventDefault();
  void currentWindow.startDragging();
}
const GRIPS: [string, ResizeEdge][] = [
  ['n', 'North'], ['s', 'South'], ['e', 'East'], ['w', 'West'],
  ['ne', 'NorthEast'], ['nw', 'NorthWest'], ['se', 'SouthEast'], ['sw', 'SouthWest'],
];
function onGripDown(edge: ResizeEdge, e: PointerEvent) {
  if (e.button !== 0) return;
  e.preventDefault();
  void currentWindow.startResizeDragging(edge);
}

const closeToast = () => (ui.toast = null);
const undoFromToast = () => void undo().then(closeToast);

let intervals: ReturnType<typeof setInterval>[] = [];
onMounted(async () => {
  window.addEventListener('keydown', onKey);
  window.addEventListener('focusin', wantsKeyboard, true);
  window.addEventListener('pointerdown', wantsKeyboard, true);
  await initStore();
  ui.weeksStart = defaultWeeksStart();
  void call<DataDirInfo>('data_dir_info').then((d) => {
    if (d?.missing) notice.value = `找不到数据文件夹 ${d.missing}（同步盘未启动？），本次使用本机默认文件夹。`;
  });
  void call('desk_ready');
  await seedIfFirstRun();
  void autoPostpone();
  void updateHolidays();
  scheduleReminders();
  startClock((day, prev) => {
    // A calendar left on the desktop overnight follows the date, unless the user paged away.
    if (ui.cursor === startOfMonth(prev)) ui.cursor = startOfMonth(day);
    if (ui.weeksStart === defaultWeeksStart(prev)) ui.weeksStart = defaultWeeksStart(day);
    if (ui.selected === prev) ui.selected = day;
    void autoPostpone();
    scheduleReminders();
  });
  on<{ error: string | null }>('store://save-status', (p) => (saveError.value = p.error));
  intervals = [
    setInterval(scheduleReminders, 30 * 60_000),
    setInterval(() => void updateHolidays(), 6 * 3600_000),
  ];
  on('reminder://fired', () => scheduleReminders());
});
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey);
  window.removeEventListener('focusin', wantsKeyboard, true);
  window.removeEventListener('pointerdown', wantsKeyboard, true);
  intervals.forEach(clearInterval);
});

watch(() => [doc.tasks, doc.settings.reminders], scheduleReminders, { deep: true });
// Separate sources: a getter returning a new array would fire on every settings update.
watch(
  [() => doc.settings.calendar.weeksRows, () => doc.settings.calendar.weeksPast, () => doc.settings.calendar.weekStart],
  () => (ui.weeksStart = defaultWeeksStart()),
);
watch(
  () => storeState.recovered,
  (r) => {
    if (r === null) return;
    notice.value = r ? `数据文件已损坏，已从备份 ${r} 恢复。` : '数据文件已损坏且没有可用备份，已重新开始。';
  },
  { immediate: true },
);
</script>

<template>
  <div class="desk" :class="{ unlocked: !desk.locked, acrylic: desk.acrylic }" @pointerdown="onAdjustDown">
    <div class="card">
      <template v-if="!desk.locked">
        <div v-for="[k, edge] in GRIPS" :key="k" class="grip" :class="k" aria-hidden="true" @pointerdown.stop="onGripDown(edge, $event)" />
      </template>
      <TitleBar />
      <FInfoBar v-if="!desk.locked" class="bar" severity="info" title="调整模式">
        按住日历任意位置拖动来移动（可拖到其他屏幕），拖动四边或四角调整大小。
        <template #action>
          <FButton variant="accent" size="small" @click="patchSettings({ desk: { locked: true } })">完成</FButton>
        </template>
      </FInfoBar>
      <FInfoBar v-if="notice" class="bar" severity="warning" closable @close="notice = ''">{{ notice }}</FInfoBar>
      <FInfoBar v-if="saveError" class="bar" severity="error" title="无法保存">数据暂存在内存中，正在重试。请检查磁盘空间或是否有程序占用数据文件。（{{ saveError }}）</FInfoBar>
      <MonthView v-if="storeState.ready && view !== 'gantt'" :mode="view === 'weeks' ? 'weeks' : 'month'" />
      <GanttView v-else-if="storeState.ready" />
      <CountdownBar v-if="storeState.ready && view !== 'gantt'" />
      <SearchPanel v-if="ui.searchOpen" />
      <Transition name="toast">
        <div v-if="ui.toast" class="toast" role="status">
          <span>{{ ui.toast.text }}</span>
          <FButton v-if="ui.toast.undo && history.canUndo" size="small" variant="accent" @click="undoFromToast">撤销</FButton>
          <FButton size="icon" variant="subtle" icon="close" title="关闭" @click="closeToast" />
        </div>
      </Transition>
    </div>
  </div>
</template>

<style scoped>
.desk {
  position: fixed;
  inset: 0;
  padding: var(--desk-pad);
  box-sizing: border-box;
}
.card {
  position: relative;
  height: 100%;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 6px 8px 8px;
  border-radius: var(--desk-radius);
  background: var(--desk-bg);
  box-shadow:
    inset 0 0 0 1px var(--desk-stroke),
    var(--desk-shadow);
  overflow: hidden;
  font-size: calc(14px * var(--fs));
}
.unlocked .card {
  outline: 2px dashed var(--accent-fill);
  outline-offset: -2px;
  cursor: move;
}
/* While adjusting, the calendar ignores the mouse so a press anywhere moves the window. */
.unlocked .card > :not(.titlebar, .bar, .grip) {
  pointer-events: none;
}
.grip {
  position: absolute;
  z-index: 40;
  --g: 8px;
}
.grip.n,
.grip.s {
  left: var(--g);
  right: var(--g);
  height: var(--g);
  cursor: ns-resize;
}
.grip.e,
.grip.w {
  top: var(--g);
  bottom: var(--g);
  width: var(--g);
  cursor: ew-resize;
}
.grip.n { top: 0; }
.grip.s { bottom: 0; }
.grip.e { right: 0; }
.grip.w { left: 0; }
.grip.ne,
.grip.nw,
.grip.se,
.grip.sw {
  width: calc(var(--g) * 2);
  height: calc(var(--g) * 2);
}
.grip.ne { top: 0; right: 0; cursor: nesw-resize; }
.grip.sw { bottom: 0; left: 0; cursor: nesw-resize; }
.grip.nw { top: 0; left: 0; cursor: nwse-resize; }
.grip.se { bottom: 0; right: 0; cursor: nwse-resize; }
.bar {
  flex: none;
}
.toast {
  position: absolute;
  left: 50%;
  bottom: 14px;
  z-index: 30;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 4px 4px 14px;
  border-radius: 8px;
  background: var(--flyout-fill);
  backdrop-filter: blur(30px);
  border: 1px solid var(--card-stroke);
  box-shadow: var(--shadow-flyout);
  font-size: 13px;
  white-space: nowrap;
  --text-glow: none;
}
.toast-enter-active,
.toast-leave-active {
  transition: opacity var(--dur-normal) var(--ease), transform var(--dur-normal) var(--ease);
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translate(-50%, 8px);
}
</style>
