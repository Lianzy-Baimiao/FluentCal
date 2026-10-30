<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch, watchEffect } from 'vue';
import { FButton, FInfoBar, FSegmented } from '@/fluent';
import { call, on } from '@/api';
import { doc, initStore, patchSettings, storeState } from '@/stores/doc';
import { deskVars, startThemeSync } from '@/composables/theme';
import { startClock } from '@/composables/now';
import type { WebStatus } from '@/webApi';
import { goToday, notify, page, ui, viewTitle } from '../desk/state';
import MonthView from '../desk/MonthView.vue';
import SearchPanel from '../desk/SearchPanel.vue';
import CountdownBar from '../desk/CountdownBar.vue';
import AgendaView from './AgendaView.vue';
import TaskDetails from './TaskDetails.vue';

/** Read-only calendar for phones and other computers in the local network (WEB, 需求文档 4.10). */
const GanttView = defineAsyncComponent(() => import('../desk/gantt/GanttView.vue'));
storeState.readOnly = true;
startThemeSync();
startClock();
void initStore();

const status = ref<WebStatus>('connecting');
const pc = ref('');
const onStatus = (s: { status: WebStatus; pc: string } | null) => {
  if (!s) return;
  status.value = s.status;
  pc.value = s.pc;
};
on<{ status: WebStatus; pc: string }>('web://status', onStatus);
void call<{ status: WebStatus; pc: string }>('web_status').then(onStatus);

// Phones start on the list; the choice is kept in this browser.
const VIEW_KEY = 'fluentcal.web.view';
const narrow = ref(matchMedia('(max-width: 640px)').matches);
const mq = matchMedia('(max-width: 640px)');
const onMq = () => (narrow.value = mq.matches);
const chosen = ref(localStorage.getItem(VIEW_KEY) ?? '');
const view = computed(() => chosen.value || (narrow.value ? 'list' : doc.settings.calendar.view));
const views = [
  { value: 'list', label: '列表' },
  { value: 'month', label: '月' },
  { value: 'weeks', label: '多周' },
  { value: 'gantt', label: '甘特' },
];
const viewModel = computed({
  get: () => view.value,
  set: (v: string) => {
    chosen.value = v;
    localStorage.setItem(VIEW_KEY, v);
    if (v !== 'list') patchSettings({ calendar: { view: v as 'month' | 'weeks' | 'gantt' } });
  },
});
const title = computed(() => (view.value === 'list' ? '近期' : viewTitle()));

// Opened on this PC ("在浏览器中查看"): the server there runs only after that menu item.
const onThisPc = ['127.0.0.1', 'localhost', '[::1]'].includes(location.hostname);
const reopen = '在日历上右键 ›“在浏览器中查看”重新打开';
const notice = computed(() => {
  switch (status.value) {
    case 'offline':
      return onThisPc
        ? { sev: 'warning' as const, text: `连不上 FluentCal（程序已退出或重新启动过）。显示的是上次的数据；${reopen}。` }
        : { sev: 'warning' as const, text: `连不上${pc.value ? ` ${pc.value} 上的` : '电脑上的'} FluentCal（电脑关机、程序已退出或不在同一网络）。显示的是上次的数据，恢复后会自动更新。` };
    case 'unauthorized':
      return { sev: 'error' as const, text: `访问码无效或已更换。${onThisPc ? `请${reopen}。` : '请在电脑上打开 FluentCal › 设置 › 网页查看，重新扫码或复制链接。'}` };
    case 'noToken':
      return { sev: 'error' as const, text: `缺少访问码。${onThisPc ? `请${reopen}。` : '请在电脑上打开 FluentCal › 设置 › 网页查看，扫码或复制完整的链接。'}` };
    default:
      return null;
  }
});

// The views are shared with the desk calendar; on a page they sit on a solid background.
watchEffect(() => {
  const root = document.documentElement;
  const vars = { ...deskVars.value, '--desk-bg': 'transparent', '--desk-shadow': 'none', '--desk-pad': '0px', '--text-glow': 'none' };
  for (const [k, v] of Object.entries(vars)) root.style.setProperty(k, v);
});
watch(() => storeState.blocked, () => notify('网页中只能查看，请在电脑上修改'));

function onKey(e: KeyboardEvent) {
  if ((e.target as HTMLElement).matches?.('input, textarea')) return;
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') ui.searchOpen = true;
  else if (e.key === 'Escape' && ui.searchOpen) ui.searchOpen = false;
  else if (view.value === 'month' || view.value === 'weeks') {
    if (e.key === 'Home') goToday();
    else if (e.key === 'PageUp' || e.key === 'PageDown') page(e.key === 'PageUp' ? -1 : 1);
    else return;
  } else return;
  e.preventDefault();
}
onMounted(() => {
  window.addEventListener('keydown', onKey);
  mq.addEventListener('change', onMq);
});
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey);
  mq.removeEventListener('change', onMq);
});
</script>

<template>
  <div class="web" :class="{ narrow }">
    <header class="bar">
      <h1 class="title">{{ title }}</h1>
      <template v-if="view === 'month' || view === 'weeks'">
        <FButton variant="subtle" size="icon" icon="chevronLeft" :title="view === 'weeks' ? '上一周' : '上个月'" @click="page(-1)" />
        <FButton variant="subtle" size="icon" icon="chevronRight" :title="view === 'weeks' ? '下一周' : '下个月'" @click="page(1)" />
        <FButton variant="subtle" size="small" @click="goToday">今天</FButton>
      </template>
      <span class="grow" />
      <FSegmented v-model="viewModel" :options="views" size="small" label="视图" />
      <FButton variant="subtle" size="icon" icon="search" title="搜索" @click="ui.searchOpen = !ui.searchOpen" />
    </header>
    <FInfoBar v-if="notice" class="notice" :severity="notice.sev">{{ notice.text }}</FInfoBar>
    <main class="body">
      <p v-if="!storeState.ready" class="loading">{{ status === 'connecting' ? '正在连接电脑上的 FluentCal…' : '' }}</p>
      <template v-else>
        <AgendaView v-if="view === 'list'" />
        <GanttView v-else-if="view === 'gantt'" />
        <MonthView v-else :mode="view === 'weeks' ? 'weeks' : 'month'" />
        <CountdownBar v-if="view !== 'gantt'" />
      </template>
      <SearchPanel v-if="ui.searchOpen && storeState.ready" />
      <Transition name="toast">
        <div v-if="ui.toast" class="toast" role="status">{{ ui.toast.text }}</div>
      </Transition>
    </main>
    <footer class="foot">只读 · {{ pc ? `来自 ${pc} 上的 FluentCal` : 'FluentCal' }}</footer>
    <TaskDetails />
  </div>
</template>

<style scoped>
.web {
  position: fixed;
  inset: 0;
  display: flex;
  flex-direction: column;
  background: var(--solid-base);
  color: var(--text-primary);
  font-size: calc(14px * var(--fs));
}
.bar {
  flex: none;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 8px 10px 4px;
  flex-wrap: wrap;
}
.title {
  margin: 0 6px 0 2px;
  font-family: var(--font-display);
  font-size: 20px;
  font-weight: 600;
  white-space: nowrap;
}
.grow {
  flex: 1;
}
.notice {
  margin: 0 10px 4px;
}
.body {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 0 8px 4px;
}
.loading {
  margin: auto;
  color: var(--text-secondary);
}
.foot {
  flex: none;
  padding: 2px 12px 6px;
  font-size: 11px;
  color: var(--text-secondary);
  text-align: center;
}
.toast {
  position: absolute;
  left: 50%;
  bottom: 16px;
  z-index: 30;
  transform: translateX(-50%);
  padding: 8px 14px;
  border-radius: 8px;
  background: var(--flyout-fill);
  border: 1px solid var(--card-stroke);
  box-shadow: var(--shadow-flyout);
  font-size: 13px;
  white-space: nowrap;
}
.toast-enter-active,
.toast-leave-active {
  transition: opacity var(--dur-normal) var(--ease);
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
}
.narrow .title {
  font-size: 18px;
}
</style>
