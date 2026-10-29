<script setup lang="ts">
import { computed, ref } from 'vue';
import { useFieldName, useSplitAttrs } from './field';

defineOptions({ inheritAttrs: false });

const props = withDefaults(
  defineProps<{
    modelValue: number;
    min?: number;
    max?: number;
    step?: number;
    width?: number | string;
    format?: (v: number) => string;
    showValue?: boolean;
    disabled?: boolean;
    label?: string;
  }>(),
  { min: 0, max: 100, step: 1, width: 200 },
);
const emit = defineEmits<{ 'update:modelValue': [number]; change: [number] }>();
const name = useFieldName(() => props.label, () => '滑块');
const split = useSplitAttrs();

const track = ref<HTMLElement>();
const dragging = ref(false);
const pct = computed(() => {
  const span = props.max - props.min || 1;
  return Math.min(100, Math.max(0, ((props.modelValue - props.min) / span) * 100));
});
const text = computed(() => (props.format ? props.format(props.modelValue) : String(props.modelValue)));

function snap(v: number): number {
  const s = props.step > 0 ? props.step : 1;
  const r = Math.round((v - props.min) / s) * s + props.min;
  const digits = (String(s).split('.')[1] ?? '').length;
  return Math.min(props.max, Math.max(props.min, Number(r.toFixed(digits))));
}

function fromPointer(e: PointerEvent): number {
  const r = track.value!.getBoundingClientRect();
  const t = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
  return snap(props.min + t * (props.max - props.min));
}

function set(v: number) {
  if (v !== props.modelValue) emit('update:modelValue', v);
}

function down(e: PointerEvent) {
  if (props.disabled || e.button !== 0) return;
  dragging.value = true;
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  (e.currentTarget as HTMLElement).focus();
  set(fromPointer(e));
}
function move(e: PointerEvent) {
  if (dragging.value) set(fromPointer(e));
}
function up() {
  if (!dragging.value) return;
  dragging.value = false;
  emit('change', props.modelValue);
}

function key(e: KeyboardEvent) {
  const big = Math.max(props.step, (props.max - props.min) / 10);
  const map: Record<string, number> = {
    ArrowRight: props.step, ArrowUp: props.step, ArrowLeft: -props.step, ArrowDown: -props.step,
    PageUp: big, PageDown: -big,
  };
  let v: number | undefined;
  if (e.key in map) v = snap(props.modelValue + map[e.key]);
  else if (e.key === 'Home') v = props.min;
  else if (e.key === 'End') v = props.max;
  if (v === undefined) return;
  e.preventDefault();
  set(v);
  emit('change', v);
}
</script>

<template>
  <div class="f-slider" :class="{ disabled, dragging }" v-bind="split.root()">
    <div
      ref="track"
      class="hit"
      role="slider"
      :tabindex="disabled ? -1 : 0"
      :aria-valuemin="min"
      :aria-valuemax="max"
      :aria-valuenow="modelValue"
      :aria-valuetext="text"
      :aria-disabled="disabled"
      v-bind="{ ...split.control(), ...name.attrs() }"
      :style="{ width: typeof width === 'number' ? `${width}px` : width }"
      @pointerdown="down"
      @pointermove="move"
      @pointerup="up"
      @pointercancel="up"
      @keydown="key"
    >
      <div class="rail"><div class="fill" :style="{ width: `${pct}%` }" /></div>
      <div class="thumb" :style="{ left: `${pct}%` }">
        <div v-if="dragging" class="tip">{{ text }}</div>
      </div>
    </div>
    <span v-if="showValue" class="val">{{ text }}</span>
  </div>
</template>

<style scoped>
.f-slider { display: inline-flex; align-items: center; gap: 12px; }
.hit { position: relative; height: 32px; touch-action: none; border-radius: var(--radius-control); }
.rail {
  position: absolute;
  left: 0;
  right: 0;
  top: 50%;
  height: 4px;
  margin-top: -2px;
  border-radius: 2px;
  background: var(--control-strong-fill);
}
.fill { height: 100%; border-radius: 2px; background: var(--accent-fill); }
.thumb {
  position: absolute;
  top: 50%;
  width: 20px;
  height: 20px;
  margin: -10px 0 0 -10px;
  border-radius: 50%;
  background: var(--control-solid-fill);
  box-shadow: 0 0 0 1px var(--control-stroke), 0 1px 0 1px var(--elev-bottom);
}
.thumb::after {
  content: '';
  position: absolute;
  inset: 0;
  margin: auto;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--accent-fill);
  transition: width var(--dur-fast) var(--ease), height var(--dur-fast) var(--ease);
}
.hit:hover .thumb::after { width: 14px; height: 14px; }
.dragging .thumb::after { width: 10px; height: 10px; background: var(--accent-fill-pressed); }
.tip {
  position: absolute;
  bottom: 28px;
  left: 50%;
  transform: translateX(-50%);
  padding: 5px 8px 7px;
  border-radius: var(--radius-control);
  background: var(--tooltip-fill);
  border: 1px solid var(--surface-stroke-flyout);
  box-shadow: var(--shadow-tooltip);
  font-size: 12px;
  line-height: 16px;
  white-space: nowrap;
  pointer-events: none;
}
.val { min-width: 3em; color: var(--text-secondary); font-variant-numeric: tabular-nums; }
.disabled .rail { background: var(--control-strong-fill-disabled); }
.disabled .fill, .disabled .thumb::after { background: var(--accent-fill-disabled); }
.disabled .val { color: var(--text-disabled); }
</style>
