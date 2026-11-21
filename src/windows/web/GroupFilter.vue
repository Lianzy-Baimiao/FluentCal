<script setup lang="ts">
import { t as tr } from '@/i18n';
import { computed, ref } from 'vue';
import { FButton, FCheckbox, FFlyout, taskColor } from '@/fluent';
import { filterRows, filterTip, toggleFilter } from '@/lib/filter';
import { doc, taskList } from '@/stores/doc';
import { isDark } from '@/composables/theme';
import { groupFilter, ui } from '../desk/state';

/**
 * 按分组筛选 in the web view (CAL-20): the button in the header and its list, as on the PC
 * (without 管理分组: the page only looks). The month, the weeks, the list and "+N" follow it;
 * it is kept on this page only, until it is loaded again.
 */
const open = ref(false);
const anchor = ref<HTMLElement | null>(null);
const on = computed(() => groupFilter.value.length > 0);
const rows = computed(() => filterRows(taskList.value, doc.groups, groupFilter.value));
const tip = computed(() => filterTip(doc.groups, groupFilter.value) || tr("按分组筛选"));
const dot = (color: string) => taskColor(color, isDark.value).bar;

function toggle(id: string, v: boolean) {
  ui.groupFilter = toggleFilter(doc.groups, groupFilter.value, id, v);
}
</script>

<template>
  <span ref="anchor" class="filter" :class="{ on }">
    <FButton variant="subtle" size="icon" icon="filter" :title="tip" :pressed="on" @click="open = !open" />
  </span>
  <FFlyout v-model:open="open" :anchor="anchor" placement="bottom-end" :width="244" :padding="false" :label="tr('按分组筛选')">
    <div class="panel">
      <div class="hd">{{ tr("按分组筛选") }}</div>
      <label class="row">
        <FCheckbox :model-value="!on" :size="18" :aria-label="tr('全部')" @update:model-value="ui.groupFilter = []" />
        <span class="name">{{ tr("全部") }}</span>
        <span class="count">{{ taskList.length }}</span>
      </label>
      <div class="sep" role="separator" />
      <div class="list">
        <label v-for="r in rows" :key="r.id" class="row">
          <FCheckbox :model-value="r.checked" :size="18" :aria-label="r.name" @update:model-value="toggle(r.id, $event)" />
          <span v-if="r.color" class="dot" :style="{ background: dot(r.color) }" aria-hidden="true" />
          <span class="name">{{ r.name }}</span>
          <span class="count">{{ r.count }}</span>
        </label>
      </div>
      <p v-if="rows.length === 1" class="hint">{{ tr("还没有分组") }}</p>
    </div>
  </FFlyout>
</template>

<style scoped>
.filter {
  display: inline-flex;
  flex: none;
}
/* red, on a tinted ground, while some tasks are hidden */
.filter.on :deep(.f-btn) {
  color: var(--critical);
}
.panel {
  padding: 5px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.hd {
  height: 28px;
  padding: 0 8px;
  display: flex;
  align-items: center;
  font-weight: 600;
}
.row {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 32px;
  padding: 0 8px;
  border-radius: var(--radius-control);
  cursor: pointer;
}
.row:hover {
  background: var(--subtle-fill-hover);
}
.sep {
  height: 1px;
  background: var(--divider);
}
.list {
  max-height: min(272px, 50vh);
  overflow-y: auto;
}
.dot {
  width: 10px;
  height: 10px;
  flex: none;
  border-radius: 3px;
}
.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.count {
  flex: none;
  font-size: 12px;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}
.hint {
  margin: 0;
  padding: 0 8px 4px;
  font-size: 12px;
  color: var(--text-tertiary);
}
</style>
