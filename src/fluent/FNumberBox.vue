<script setup lang="ts">
import { ref, watch } from 'vue';
import FIcon from './FIcon.vue';
import { useFieldName, useSplitAttrs } from './field';

defineOptions({ inheritAttrs: false });

const props = withDefaults(
  defineProps<{
    modelValue: number;
    min?: number;
    max?: number;
    step?: number;
    suffix?: string;
    width?: number | string;
    disabled?: boolean;
    label?: string;
  }>(),
  { min: -Infinity, max: Infinity, step: 1, width: 120 },
);
const emit = defineEmits<{ 'update:modelValue': [number] }>();
const name = useFieldName(() => props.label, () => '数值');
const split = useSplitAttrs();

const text = ref(String(props.modelValue));
watch(() => props.modelValue, (v) => (text.value = String(v)));

function clamp(v: number): number {
  const digits = (String(props.step).split('.')[1] ?? '').length;
  return Math.min(props.max, Math.max(props.min, Number(v.toFixed(digits))));
}

function commit() {
  const v = Number(text.value.replace(/[^\d.\-]/g, ''));
  if (text.value.trim() === '' || Number.isNaN(v)) {
    text.value = String(props.modelValue);
    return;
  }
  const c = clamp(v);
  text.value = String(c);
  if (c !== props.modelValue) emit('update:modelValue', c);
}

function stepBy(n: number) {
  if (props.disabled) return;
  const c = clamp(props.modelValue + n * props.step);
  if (c !== props.modelValue) emit('update:modelValue', c);
}

function key(e: KeyboardEvent) {
  if (e.key === 'ArrowUp') { e.preventDefault(); stepBy(1); }
  else if (e.key === 'ArrowDown') { e.preventDefault(); stepBy(-1); }
  else if (e.key === 'PageUp') { e.preventDefault(); stepBy(10); }
  else if (e.key === 'PageDown') { e.preventDefault(); stepBy(-10); }
  else if (e.key === 'Enter') commit();
}
</script>

<template>
  <div class="f-num" :class="{ disabled }" :style="{ width: typeof width === 'number' ? `${width}px` : width }" v-bind="split.root()">
    <input
      v-model="text"
      type="text"
      inputmode="decimal"
      role="spinbutton"
      :aria-valuenow="modelValue"
      :aria-valuemin="Number.isFinite(min) ? min : undefined"
      :aria-valuemax="Number.isFinite(max) ? max : undefined"
      :disabled="disabled"
      v-bind="{ ...split.control(), ...name.attrs() }"
      @blur="commit"
      @keydown="key"
    />
    <span v-if="suffix" class="suffix">{{ suffix }}</span>
    <button type="button" class="spin" tabindex="-1" aria-label="增加" :disabled="disabled || modelValue >= max" @click="stepBy(1)">
      <FIcon name="chevronUp" :size="10" />
    </button>
    <button type="button" class="spin" tabindex="-1" aria-label="减少" :disabled="disabled || modelValue <= min" @click="stepBy(-1)">
      <FIcon name="chevronDown" :size="10" />
    </button>
  </div>
</template>

<style scoped>
.f-num {
  position: relative;
  display: inline-flex;
  align-items: center;
  height: 32px;
  border-radius: var(--radius-control);
  background: var(--control-fill);
  border: 1px solid var(--control-stroke);
  border-bottom-color: var(--control-strong-stroke);
  overflow: hidden;
}
.f-num::after {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  bottom: -1px;
  height: 2px;
  background: var(--accent-fill);
  transform: scaleX(0);
  transition: transform var(--dur-fast) var(--ease);
}
.f-num:hover:not(.disabled) { background: var(--control-fill-hover); }
.f-num:focus-within { background: var(--control-fill-input-active); }
.f-num:focus-within::after { transform: scaleX(1); }
input {
  flex: 1;
  min-width: 0;
  border: 0;
  outline: none;
  background: transparent;
  padding: 5px 4px 6px 11px;
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}
input:focus-visible { outline: none; box-shadow: none; }
.suffix { color: var(--text-secondary); padding-right: 4px; }
.spin {
  width: 24px;
  height: 24px;
  padding: 0;
  border: 0;
  border-radius: var(--radius-control);
  background: transparent;
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
}
.spin:last-child { margin-right: 3px; }
.spin:hover:not(:disabled) { background: var(--subtle-fill-hover); }
.spin:active:not(:disabled) { background: var(--subtle-fill-pressed); }
.spin:disabled { color: var(--text-disabled); }
.disabled { background: var(--control-fill-disabled); border-color: var(--control-stroke); }
.disabled input { color: var(--text-disabled); }
</style>
