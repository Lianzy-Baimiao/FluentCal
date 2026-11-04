<script setup lang="ts">
import { t as tr } from '@/i18n';
import { computed, nextTick, onMounted, reactive, ref } from 'vue';
import { FButton, FColorSwatches, FSelect, FTextBox, TASK_COLORS, taskColor } from '@/fluent';
import { searchTasks, type SearchOptions } from '@/lib/search';
import { formatFriendly, formatRange } from '@/lib/date';
import { doc, taskList } from '@/stores/doc';
import { now } from '@/composables/now';
import { isDark } from '@/composables/theme';
import { jumpTo, openEditor, ui } from './state';

const o = reactive<SearchOptions>({ q: '', range: 'all', status: 'all', color: '' });
const box = ref<InstanceType<typeof FTextBox>>();
const results = computed(() => (o.q.trim() || o.color || o.status !== 'all' ? searchTasks(taskList.value, doc.groups, o, now.value).slice(0, 100) : []));
const colors = computed(() => TASK_COLORS.map((c) => ({ key: c.key, color: c.bar, label: tr(c.label) })));

function go(date: string) {
  ui.searchOpen = false;
  jumpTo(date);
}

onMounted(() => void nextTick(() => box.value?.focus()));
</script>

<template>
  <div class="search" role="dialog" :aria-label="tr('搜索任务')" style="--text-glow: none" @keydown.esc.stop="ui.searchOpen = false">
    <div class="bar">
      <FTextBox ref="box" v-model="o.q" icon="search" :placeholder="tr('搜索任务、备注或分组')" clearable @escape="ui.searchOpen = false" @enter="results[0] && go(results[0].date)" />
      <FButton variant="subtle" size="icon" icon="close" :title="tr('关闭 (Esc)')" @click="ui.searchOpen = false" />
    </div>
    <div class="filters">
      <FSelect
        v-model="o.range"
        :width="104"
        :options="[
          { value: 'all', label: tr('全部时间') },
          { value: '1m', label: tr('前后一月') },
          { value: '3m', label: tr('前后三月') },
          { value: '1y', label: tr('前后一年') },
        ]"
      />
      <FSelect
        v-model="o.status"
        :width="96"
        :options="[
          { value: 'all', label: tr('全部') },
          { value: 'undone', label: tr('未完成') },
          { value: 'done', label: tr('已完成') },
        ]"
      />
      <FColorSwatches v-model="o.color" :colors="colors" allow-none :size="18" />
    </div>
    <ul v-if="results.length" class="results" role="listbox" :aria-label="tr('搜索结果')">
      <li v-for="h in results" :key="h.task.id" role="option" tabindex="0" @click="go(h.date)" @keydown.enter="go(h.date)" @dblclick="openEditor({ mode: 'edit', taskId: h.task.id, date: h.date })">
        <span class="dot" :style="{ background: taskColor(h.task.color, isDark).bar }" />
        <span class="t" :class="{ done: h.task.done }">{{ h.task.title }}</span>
        <span class="d">{{ h.task.repeat ? tr("{0}（重复）", formatFriendly(h.date)) : formatRange(h.task.start, h.task.end) }}</span>
      </li>
    </ul>
    <p v-else-if="o.q.trim()" class="empty">{{ tr("没有找到相关任务") }}</p>
    <p v-else class="empty">{{ tr("输入关键字，回车跳转到第一个结果") }}</p>
  </div>
</template>

<style scoped>
.search {
  position: absolute;
  z-index: 20;
  top: 44px;
  right: 8px;
  width: min(360px, calc(100% - 16px));
  max-height: calc(100% - 56px);
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  box-sizing: border-box;
  border-radius: 8px;
  background: var(--flyout-fill);
  backdrop-filter: blur(30px) saturate(1.2);
  border: 1px solid var(--card-stroke);
  box-shadow: var(--shadow-flyout);
}
.bar {
  display: flex;
  gap: 4px;
}
.bar > :first-child {
  flex: 1;
  min-width: 0;
}
.filters {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}
.results {
  list-style: none;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  min-height: 0;
}
.results li {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 32px;
  padding: 0 8px;
  border-radius: 4px;
  cursor: pointer;
}
.results li:hover {
  background: var(--subtle-fill-hover);
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex: none;
}
.t {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.t.done {
  text-decoration: line-through;
  color: var(--text-secondary);
}
.d {
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
}
.empty {
  margin: 4px 2px;
  font-size: 12px;
  color: var(--text-tertiary);
}
</style>
