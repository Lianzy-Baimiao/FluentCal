<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { FButton, FIcon, showMenu } from '@/fluent';
import { call, isTauri, on } from '@/api';
import { doc, initStore } from '@/stores/doc';
import { startThemeSync } from '@/composables/theme';
import { setOccurrenceDone } from '@/lib/tasks';
import type { ReminderItem } from '@/lib/reminders';
import '../window-shell.css';

startThemeSync();
const items = ref<ReminderItem[]>([]);
/** Newest first; the window is sized for three cards, the rest scroll. */
const shown = computed(() => [...items.value].reverse());

async function refresh() {
  if (!isTauri) return void (items.value = demo());
  const list = await call<ReminderItem[]>('reminder_popup_list');
  // A snoozed reminder may belong to a task that was completed or deleted meanwhile.
  const stale = list.filter((r) => !doc.tasks[r.taskId] || (doc.tasks[r.taskId].done && !doc.tasks[r.taskId].repeat));
  for (const r of stale) void call('reminder_popup_dismiss', { key: r.key });
  items.value = list.filter((r) => !stale.includes(r));
}

const dismiss = (r: ReminderItem) => {
  items.value = items.value.filter((x) => x.key !== r.key);
  void call('reminder_popup_dismiss', { key: r.key });
};
const snooze = (r: ReminderItem, minutes: number) => {
  items.value = items.value.filter((x) => x.key !== r.key);
  void call('reminder_snooze', { key: r.key, minutes });
};
async function done(r: ReminderItem) {
  const t = doc.tasks[r.taskId];
  if (t) await setOccurrenceDone(t, r.date, true);
  dismiss(r);
}
function later(r: ReminderItem, e: MouseEvent) {
  const opts: [number, string][] = [[5, '5 分钟后'], [30, '30 分钟后'], [60, '1 小时后'], [180, '3 小时后']];
  void showMenu(opts.map(([m, label]) => ({ label, action: () => snooze(r, m) })), e.currentTarget as HTMLElement, { placement: 'bottom-end' });
}

/** Browser preview: two sample cards. */
function demo(): ReminderItem[] {
  const id = Object.keys(doc.tasks)[0] ?? 'demo';
  return [
    { key: 'a', atMs: Date.now(), title: '交周报', body: '今天 17:00 · 提前 15 分钟', taskId: id, date: new Date().toISOString().slice(0, 10) },
    { key: 'b', atMs: Date.now(), title: '项目评审会', body: '今天 15:00', taskId: id, date: new Date().toISOString().slice(0, 10) },
  ];
}

onMounted(async () => {
  await initStore();
  await refresh();
  on('reminder://popup', () => void refresh());
});
</script>

<template>
  <div class="popup" role="alertdialog" aria-label="任务提醒">
    <section v-for="r in shown" :key="r.key" class="card" role="group" :aria-label="r.title">
      <header>
        <FIcon name="alarm" :size="14" class="ic" />
        <span class="app">FluentCal 提醒</span>
        <button class="x" type="button" aria-label="关闭提醒" @click="dismiss(r)"><FIcon name="close" :size="10" /></button>
      </header>
      <div class="title">{{ r.title }}</div>
      <div class="body">{{ r.body }}</div>
      <footer>
        <FButton size="small" variant="accent" icon="checkMark" @click="done(r)">完成</FButton>
        <FButton size="small" @click="snooze(r, 10)">10 分钟后提醒</FButton>
        <FButton size="small" variant="subtle" icon="chevronDown" title="其他时间" aria-label="选择稍后提醒的时间" @click="later(r, $event)" />
      </footer>
    </section>
  </div>
</template>

<style scoped>
.popup {
  position: fixed;
  inset: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 4px;
  overflow-y: auto;
  box-sizing: border-box;
}
.card {
  flex: none;
}
/* Stack at the bottom; unlike justify-content: flex-end this keeps overflow scrollable. */
.card:first-child {
  margin-top: auto;
}
.card {
  padding: 10px 12px 12px;
  border-radius: 8px;
  /* Nearly opaque: the popup window is transparent and floats over other apps. */
  background: color-mix(in srgb, var(--solid-base) 97%, transparent);
  border: 1px solid var(--card-stroke);
  box-shadow: var(--shadow-flyout);
  color: var(--text-primary);
}
header {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-secondary);
}
.ic {
  color: var(--accent-text);
}
.app {
  flex: 1;
}
.x {
  width: 24px;
  height: 24px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-secondary);
  display: grid;
  place-items: center;
  cursor: pointer;
}
.x:hover {
  background: var(--subtle-fill-hover);
}
.title {
  margin-top: 6px;
  font-size: 14px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.body {
  margin-top: 2px;
  font-size: 12px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
footer {
  display: flex;
  gap: 6px;
  margin-top: 10px;
}
</style>
