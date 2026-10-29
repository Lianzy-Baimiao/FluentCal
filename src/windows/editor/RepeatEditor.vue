<script setup lang="ts">
import { computed } from 'vue';
import { FDatePicker, FNumberBox, FRadioGroup, FSelect } from '@/fluent';
import type { ISODate, RepeatFreq, RepeatRule } from '@/types';
import { addMonths, daysInMonth, parts, weekday, WEEKDAY_SHORT } from '@/lib/date';
import { describeRule } from '@/lib/recur';

const props = defineProps<{ start: ISODate }>();
const rule = defineModel<RepeatRule | undefined>();

const FREQS: { value: RepeatFreq | ''; label: string }[] = [
  { value: '', label: '不重复' },
  { value: 'daily', label: '每天' },
  { value: 'weekly', label: '每周' },
  { value: 'monthly', label: '每月' },
  { value: 'yearly', label: '每年' },
  { value: 'workday', label: '工作日（周一至周五）' },
  { value: 'legalWorkday', label: '法定工作日（含调休）' },
  { value: 'lunarYearly', label: '农历每年' },
  { value: 'monthLastDay', label: '每月最后一天' },
  { value: 'ebbinghaus', label: '艾宾浩斯记忆曲线' },
];
const UNIT: Partial<Record<RepeatFreq, string>> = { daily: '天', weekly: '周', monthly: '个月', yearly: '年', lunarYearly: '年', monthLastDay: '个月' };

const freq = computed({
  get: () => rule.value?.freq ?? '',
  set: (f: RepeatFreq | '') => (rule.value = f ? { freq: f, interval: 1, weekdays: f === 'weekly' ? [weekday(props.start)] : undefined } : undefined),
});
const patch = (p: Partial<RepeatRule>) => rule.value && (rule.value = { ...rule.value, ...p });
const interval = computed({ get: () => rule.value?.interval ?? 1, set: (v: number) => patch({ interval: Math.max(1, Math.min(99, v || 1)) }) });

const order = [1, 2, 3, 4, 5, 6, 0];
function toggleDay(d: number) {
  const set = new Set(rule.value?.weekdays ?? []);
  if (set.has(d) && set.size > 1) set.delete(d);
  else set.add(d);
  patch({ weekdays: [...set].sort() });
}

const monthModes = computed(() => {
  const [y, m, d] = parts(props.start);
  const nth = Math.ceil(d / 7);
  const last = nth >= 5 || d + 7 > daysInMonth(y, m);
  return [
    { value: 'day', label: `每月 ${d} 日` },
    { value: 'nthWeekday', label: `每月${last ? '最后一个' : `第 ${nth} 个`}周${WEEKDAY_SHORT[weekday(props.start)]}` },
  ];
});
const monthMode = computed({ get: () => rule.value?.monthMode ?? 'day', set: (v: string) => patch({ monthMode: v as 'day' }) });

const endMode = computed({
  get: () => (rule.value?.count ? 'count' : rule.value?.until ? 'until' : 'never'),
  set: (v: string) => patch({ count: v === 'count' ? 10 : undefined, until: v === 'until' ? addMonths(props.start, 3) : undefined }),
});
const count = computed({ get: () => rule.value?.count ?? 10, set: (v: number) => patch({ count: Math.max(1, Math.min(999, v || 1)) }) });
const until = computed({ get: () => rule.value?.until ?? '', set: (v: string) => patch({ until: v || undefined }) });
const summary = computed(() => describeRule(rule.value, props.start));
</script>

<template>
  <div class="repeat">
    <div class="line">
      <FSelect v-model="freq" :options="FREQS" :width="200" />
      <template v-if="rule && UNIT[rule.freq]">
        <span>每</span>
        <FNumberBox v-model="interval" :min="1" :max="99" :width="100" />
        <span>{{ UNIT[rule.freq] }}</span>
      </template>
    </div>
    <div v-if="rule?.freq === 'weekly'" class="days" role="group" aria-label="重复的星期">
      <button
        v-for="d in order"
        :key="d"
        type="button"
        :class="{ on: rule.weekdays?.includes(d) }"
        :aria-pressed="rule.weekdays?.includes(d) ?? false"
        @click="toggleDay(d)"
      >
        {{ WEEKDAY_SHORT[d] }}
      </button>
    </div>
    <FRadioGroup v-if="rule?.freq === 'monthly'" v-model="monthMode" :options="monthModes" direction="horizontal" />
    <div v-if="rule && rule.freq !== 'ebbinghaus'" class="line">
      <FSelect
        v-model="endMode"
        :width="140"
        :options="[
          { value: 'never', label: '永不结束' },
          { value: 'count', label: '重复次数' },
          { value: 'until', label: '截止日期' },
        ]"
      />
      <FNumberBox v-if="endMode === 'count'" v-model="count" :min="1" :max="999" suffix="次" :width="96" />
      <FDatePicker v-if="endMode === 'until'" v-model="until" :width="150" />
    </div>
    <p v-if="rule" class="summary">{{ summary }}</p>
  </div>
</template>

<style scoped>
.repeat {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.line {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.days {
  display: flex;
  gap: 4px;
}
.days button {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  border: 1px solid var(--control-stroke);
  background: var(--control-fill);
  color: var(--text-primary);
  font: inherit;
  cursor: pointer;
}
.days button:hover {
  background: var(--control-fill-hover);
}
.days button.on {
  background: var(--accent-fill);
  border-color: var(--accent-fill);
  color: var(--text-on-accent);
}
.summary {
  margin: 0;
  font-size: 12px;
  color: var(--text-secondary);
}
</style>
