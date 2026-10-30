<script setup lang="ts">
import { computed, onMounted, provide, ref, type Component } from 'vue';
import { FNavView } from '@/fluent';
import { call, currentWindow, on } from '@/api';
import { initStore, storeState } from '@/stores/doc';
import { startThemeSync } from '@/composables/theme';
import GeneralPage from './GeneralPage.vue';
import AppearancePage from './AppearancePage.vue';
import CalendarPage from './CalendarPage.vue';
import GanttPage from './GanttPage.vue';
import RemindersPage from './RemindersPage.vue';
import DataPage from './DataPage.vue';
import SyncPage from './SyncPage.vue';
import AboutPage from './AboutPage.vue';
import '../window-shell.css';
import './settings.css';

startThemeSync();

const items = [
  { key: 'general', label: '常规', icon: 'settings' },
  { key: 'appearance', label: '外观', icon: 'palette' },
  { key: 'calendar', label: '日历', icon: 'calendar' },
  { key: 'gantt', label: '甘特图', icon: 'gantt' },
  { key: 'reminders', label: '提醒', icon: 'alarm' },
  { key: 'data', label: '数据', icon: 'data' },
  { key: 'sync', label: '同步', icon: 'cloud' },
];
const footer = [{ key: 'about', label: '关于', icon: 'info' }];
const pages: Record<string, Component> = {
  general: GeneralPage,
  appearance: AppearancePage,
  calendar: CalendarPage,
  gantt: GanttPage,
  reminders: RemindersPage,
  data: DataPage,
  sync: SyncPage,
  about: AboutPage,
};
const pageKey = ref('general');
const page = computed(() => pages[pageKey.value] ?? GeneralPage);
/** Lets a page send the user to another one ("set the data folder on the Data page"). */
provide('settingsNav', (key: string) => {
  if (key in pages) pageKey.value = key;
});

async function takePending() {
  const p = await call<{ page?: string } | null>('pending_take');
  if (p?.page && p.page in pages) pageKey.value = p.page;
}

onMounted(async () => {
  await initStore();
  await takePending();
  on('pending://settings', () => void takePending());
  await currentWindow.show();
  await currentWindow.focus();
});
</script>

<template>
  <div class="aux settings">
    <FNavView v-model="pageKey" :items="items" :footer-items="footer" title="设置">
      <div class="page-scroll">
        <component :is="page" v-if="storeState.ready" :key="pageKey" />
      </div>
    </FNavView>
  </div>
</template>
