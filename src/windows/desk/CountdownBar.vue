<script setup lang="ts">
import { computed } from 'vue';
import { FIcon, vTip } from '@/fluent';
import { countdowns } from '@/lib/countdown';
import { formatFriendly } from '@/lib/date';
import { doc, taskList } from '@/stores/doc';
import { now } from '@/composables/now';
import { jumpTo } from './state';

const items = computed(() => countdowns(taskList.value, now.value, doc.settings.calendar.holidayCountdown).slice(0, 6));
</script>

<template>
  <div v-if="items.length" class="countdown" role="group" aria-label="倒数日">
    <button v-for="c in items" :key="c.key" type="button" class="cd" v-tip="formatFriendly(c.date)" @click="jumpTo(c.date)">
      <FIcon :name="c.taskId ? 'flag' : 'calendarDay'" :size="11" />
      <template v-if="c.days === 0"><b>{{ c.label }}</b> 就是今天</template>
      <template v-else>距<b>{{ c.label }}</b>还有 <em>{{ c.days }}</em> 天</template>
    </button>
  </div>
</template>

<style scoped>
.countdown {
  flex: none;
  display: flex;
  gap: 4px;
  overflow: hidden;
  height: calc(22px * var(--fs));
}
.cd {
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 0 8px;
  border: 0;
  border-radius: 11px;
  background: var(--cell-bg);
  color: var(--text-secondary);
  font: inherit;
  font-size: calc(12px * var(--fs));
  white-space: nowrap;
  text-shadow: var(--text-glow);
  cursor: pointer;
}
.cd:hover {
  background: var(--subtle-fill-hover);
  color: var(--text-primary);
}
.cd b {
  font-weight: 600;
  color: var(--text-primary);
  margin: 0 2px;
}
.cd em {
  font-style: normal;
  font-weight: 700;
  color: var(--accent-text);
  font-variant-numeric: tabular-nums;
}
</style>
