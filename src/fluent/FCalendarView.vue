<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import FIcon from './FIcon.vue';
import { addDays, daysInMonth, iso, toDate, todayIso } from './dates';

const props = withDefaults(
  defineProps<{
    modelValue: string;
    firstDay?: 0 | 1;
    lunar?: (iso: string) => string;
    markers?: (iso: string) => string | undefined;
  }>(),
  { firstDay: 1 },
);
const emit = defineEmits<{ 'update:modelValue': [string] }>();

const today = todayIso();
const cursor = ref(props.modelValue || today);
const mode = ref<'days' | 'months'>('days');
const grid = ref<HTMLElement>();
const view = ref(ymOf(cursor.value));

function ymOf(s: string) {
  const d = toDate(s) ?? new Date();
  return { y: d.getFullYear(), m: d.getMonth() + 1 };
}

watch(() => props.modelValue, (v) => {
  if (v) {
    cursor.value = v;
    view.value = ymOf(v);
  }
});

const WEEK = ['日', '一', '二', '三', '四', '五', '六'];
const weekdays = computed(() => [...WEEK.slice(props.firstDay), ...WEEK.slice(0, props.firstDay)]);

const days = computed(() => {
  const { y, m } = view.value;
  const first = new Date(y, m - 1, 1);
  const lead = (first.getDay() - props.firstDay + 7) % 7;
  const start = addDays(iso(y, m, 1), -lead);
  return Array.from({ length: 42 }, (_, i) => {
    const d = addDays(start, i);
    return {
      iso: d,
      day: +d.slice(8),
      out: +d.slice(5, 7) !== m,
      lunar: props.lunar?.(d) ?? '',
      marker: props.markers?.(d),
    };
  });
});

function shift(months: number) {
  let { y, m } = view.value;
  m += months;
  while (m < 1) { m += 12; y--; }
  while (m > 12) { m -= 12; y++; }
  view.value = { y, m };
  const d = Math.min(+cursor.value.slice(8), daysInMonth(y, m));
  cursor.value = iso(y, m, d);
}

function pick(d: string) {
  cursor.value = d;
  const ym = ymOf(d);
  if (ym.m !== view.value.m || ym.y !== view.value.y) view.value = ym;
  emit('update:modelValue', d);
}

function pickMonth(m: number) {
  view.value = { y: view.value.y, m };
  cursor.value = iso(view.value.y, m, Math.min(+cursor.value.slice(8), daysInMonth(view.value.y, m)));
  mode.value = 'days';
}

function key(e: KeyboardEvent) {
  const map: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 };
  let next: string | null = null;
  if (e.key in map) next = addDays(cursor.value, map[e.key]);
  else if (e.key === 'PageUp' || e.key === 'PageDown') { shift(e.key === 'PageUp' ? -1 : 1); next = cursor.value; }
  else if (e.key === 'Home' || e.key === 'End') {
    const wd = ((toDate(cursor.value)!.getDay() - props.firstDay) + 7) % 7;
    next = addDays(cursor.value, e.key === 'Home' ? -wd : 6 - wd);
  } else if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pick(cursor.value); return; }
  if (!next) return;
  e.preventDefault();
  cursor.value = next;
  const ym = ymOf(next);
  if (ym.m !== view.value.m || ym.y !== view.value.y) view.value = ym;
  nextTick(() => grid.value?.querySelector<HTMLElement>(`[data-iso="${next}"]`)?.focus());
}
</script>

<template>
  <div class="f-cal" :class="{ lunar: !!lunar }">
    <div class="head">
      <button type="button" class="title" @click="mode = mode === 'days' ? 'months' : 'days'">
        {{ mode === 'days' ? `${view.y}年${view.m}月` : `${view.y}年` }}
      </button>
      <button type="button" class="nav" :aria-label="mode === 'days' ? '上个月' : '上一年'" @click="mode === 'days' ? shift(-1) : (view.y--)">
        <FIcon name="chevronUp" :size="12" />
      </button>
      <button type="button" class="nav" :aria-label="mode === 'days' ? '下个月' : '下一年'" @click="mode === 'days' ? shift(1) : (view.y++)">
        <FIcon name="chevronDown" :size="12" />
      </button>
    </div>
    <template v-if="mode === 'days'">
      <div class="week"><span v-for="w in weekdays" :key="w">{{ w }}</span></div>
      <div ref="grid" class="grid" role="group" :aria-label="`${view.y}年${view.m}月`" @keydown="key">
        <button
          v-for="d in days"
          :key="d.iso"
          type="button"
          class="day"
          :class="{ out: d.out, today: d.iso === today, sel: d.iso === modelValue }"
          :data-iso="d.iso"
          :tabindex="d.iso === cursor ? 0 : -1"
          :aria-pressed="d.iso === modelValue"
          :aria-current="d.iso === today ? 'date' : undefined"
          @click="pick(d.iso)"
        >
          <span class="visually-hidden">{{ +d.iso.slice(0, 4) }}年{{ +d.iso.slice(5, 7) }}月</span>
          <span class="num">{{ d.day }}</span>
          <span v-if="d.lunar" class="lun">{{ d.lunar }}</span>
          <span v-if="d.marker" class="mark" :class="{ off: d.marker === '休' }">{{ d.marker }}</span>
        </button>
      </div>
    </template>
    <div v-else class="months">
      <button
        v-for="m in 12"
        :key="m"
        type="button"
        class="mon"
        :class="{ cur: m === view.m }"
        @click="pickMonth(m)"
      >{{ m }}月</button>
    </div>
  </div>
</template>

<style scoped>
.f-cal { width: 300px; padding: 2px 4px 6px; }
.f-cal.lunar { width: 316px; }
.head { display: flex; align-items: center; gap: 2px; height: 40px; padding: 0 2px 0 0; }
.title {
  flex: 1;
  height: 32px;
  padding: 0 8px;
  border: 0;
  border-radius: var(--radius-control);
  background: transparent;
  text-align: left;
  font-weight: 600;
}
.title:hover, .nav:hover { background: var(--subtle-fill-hover); }
.title:active, .nav:active { background: var(--subtle-fill-pressed); }
.nav {
  width: 32px;
  height: 32px;
  border: 0;
  padding: 0;
  border-radius: var(--radius-control);
  background: transparent;
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.week, .grid { display: grid; grid-template-columns: repeat(7, 1fr); }
.week span { height: 32px; display: flex; align-items: center; justify-content: center; font-size: 12px; color: var(--text-primary); }
.day {
  position: relative;
  height: 40px;
  margin: 1px;
  padding: 0;
  border: 1px solid transparent;
  border-radius: 50%;
  background: transparent;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  line-height: 1;
}
.lunar .day { height: 42px; border-radius: 6px; }
.num { font-size: 14px; }
.lun { margin-top: 3px; font-size: 10px; color: var(--text-secondary); max-width: 100%; overflow: hidden; white-space: nowrap; }
.day:hover { background: var(--subtle-fill-hover); }
.day:active { background: var(--subtle-fill-pressed); }
.out .num, .out .lun { color: var(--text-secondary); }
.out .num { font-weight: 300; }
.today { background: var(--accent-fill); color: var(--text-on-accent); font-weight: 600; }
.today .lun { color: var(--text-on-accent); font-weight: 400; }
.today:hover { background: var(--accent-fill-hover); }
.sel { border-color: var(--accent-fill); }
.today.sel { box-shadow: inset 0 0 0 2px var(--accent-fill), inset 0 0 0 3px var(--text-on-accent); }
.mark {
  position: absolute;
  top: 1px;
  right: 2px;
  font-size: 9px;
  line-height: 10px;
  color: var(--text-secondary);
}
.mark.off { color: var(--critical); }
.today .mark { color: var(--text-on-accent); }
.months { display: grid; grid-template-columns: repeat(4, 1fr); gap: 4px; padding: 8px 2px; }
.mon {
  height: 56px;
  border: 1px solid transparent;
  border-radius: 50%;
  background: transparent;
}
.mon:hover { background: var(--subtle-fill-hover); }
.mon.cur { background: var(--accent-fill); color: var(--text-on-accent); }
</style>
