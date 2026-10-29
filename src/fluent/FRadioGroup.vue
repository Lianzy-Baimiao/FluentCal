<script setup lang="ts">
import { useFieldName } from './field';
type V = string | number;
const props = withDefaults(
  defineProps<{
    modelValue: V;
    options: { value: V; label: string; disabled?: boolean }[];
    direction?: 'vertical' | 'horizontal';
    name?: string;
    label?: string;
  }>(),
  { direction: 'vertical' },
);
const emit = defineEmits<{ 'update:modelValue': [V] }>();
const name = useFieldName(() => props.label);

function pick(v: V, disabled?: boolean) {
  if (!disabled && v !== props.modelValue) emit('update:modelValue', v);
}

function onKey(e: KeyboardEvent, i: number) {
  const delta = e.key === 'ArrowDown' || e.key === 'ArrowRight' ? 1 : e.key === 'ArrowUp' || e.key === 'ArrowLeft' ? -1 : 0;
  if (!delta) return;
  e.preventDefault();
  const n = props.options.length;
  for (let k = 1; k <= n; k++) {
    const o = props.options[(i + delta * k + n * k) % n];
    if (!o.disabled) {
      pick(o.value);
      const group = (e.currentTarget as HTMLElement).parentElement;
      requestAnimationFrame(() => group?.querySelector<HTMLElement>('[aria-checked="true"]')?.focus());
      return;
    }
  }
}
</script>

<template>
  <div class="f-radios" :class="direction" role="radiogroup" v-bind="name.attrs()">
    <button
      v-for="(o, i) in options"
      :key="String(o.value)"
      type="button"
      class="f-radio"
      :class="{ on: o.value === modelValue }"
      role="radio"
      :aria-checked="o.value === modelValue"
      :tabindex="o.value === modelValue || (i === 0 && !options.some((x) => x.value === modelValue)) ? 0 : -1"
      :disabled="o.disabled"
      @click="pick(o.value, o.disabled)"
      @keydown="onKey($event, i)"
    >
      <span class="dot" /><span class="lbl">{{ o.label }}</span>
    </button>
  </div>
</template>

<style scoped>
.f-radios { display: flex; flex-direction: column; }
.f-radios.horizontal { flex-direction: row; flex-wrap: wrap; column-gap: 24px; }
.f-radio {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-height: 32px;
  padding: 0;
  border: 0;
  background: none;
  color: var(--text-primary);
  text-align: left;
  border-radius: var(--radius-control);
}
.dot {
  position: relative;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  border: 1px solid var(--control-strong-stroke);
  background: var(--control-alt-fill);
  flex: none;
  transition: background-color var(--dur-fast) var(--ease), border-color var(--dur-fast);
}
.dot::after {
  content: '';
  position: absolute;
  left: 50%;
  top: 50%;
  width: 0;
  height: 0;
  border-radius: 50%;
  background: var(--text-on-accent);
  transform: translate(-50%, -50%);
  transition: width var(--dur-fast) var(--ease), height var(--dur-fast) var(--ease);
}
.f-radio:hover:not(:disabled) .dot { background: var(--control-alt-fill-hover); }
.f-radio:active:not(:disabled) .dot { background: var(--control-alt-fill-pressed); }
.f-radio:active:not(:disabled):not(.on) .dot::after { width: 10px; height: 10px; background: var(--text-secondary); }
.on .dot { background: var(--accent-fill); border-color: var(--accent-fill); }
.on .dot::after { width: 10px; height: 10px; }
.on:hover:not(:disabled) .dot { background: var(--accent-fill-hover); }
.on:hover:not(:disabled) .dot::after { width: 12px; height: 12px; }
.on:active:not(:disabled) .dot::after { width: 8px; height: 8px; }
.f-radio:disabled { color: var(--text-disabled); }
.f-radio:disabled .dot { border-color: var(--control-strong-stroke-disabled); }
.on:disabled .dot { background: var(--accent-fill-disabled); border-color: transparent; }
</style>
