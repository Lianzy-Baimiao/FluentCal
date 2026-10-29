<script setup lang="ts">
import { useFieldName } from './field';
import FIcon from './FIcon.vue';

const props = withDefaults(
  defineProps<{
    modelValue: string;
    options: { value: string; label?: string; icon?: string; title?: string }[];
    size?: 'small' | 'medium';
    label?: string;
  }>(),
  { size: 'medium' },
);
const emit = defineEmits<{ 'update:modelValue': [string] }>();
const name = useFieldName(() => props.label);

function key(e: KeyboardEvent) {
  const d = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
  if (!d) return;
  e.preventDefault();
  const i = props.options.findIndex((o) => o.value === props.modelValue);
  const n = props.options[(i + d + props.options.length) % props.options.length];
  emit('update:modelValue', n.value);
  const host = e.currentTarget as HTMLElement;
  requestAnimationFrame(() => host.querySelector<HTMLElement>('[aria-checked="true"]')?.focus());
}
</script>

<template>
  <div class="f-seg" :class="size" role="radiogroup" v-bind="name.attrs()" @keydown="key">
    <button
      v-for="o in options"
      :key="o.value"
      type="button"
      class="item"
      :class="{ on: o.value === modelValue, iconOnly: !o.label }"
      role="radio"
      :aria-checked="o.value === modelValue"
      :aria-label="o.label ? undefined : o.title"
      :title="o.title"
      :tabindex="o.value === modelValue ? 0 : -1"
      @click="o.value !== modelValue && emit('update:modelValue', o.value)"
    >
      <FIcon v-if="o.icon" :name="o.icon" :size="size === 'small' ? 14 : 16" />
      <span v-if="o.label">{{ o.label }}</span>
    </button>
  </div>
</template>

<style scoped>
.f-seg {
  display: inline-flex;
  gap: 2px;
  padding: 2px;
  border-radius: var(--radius-control);
  background: var(--control-alt-fill);
  border: 1px solid var(--control-stroke);
}
.item {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: 26px;
  padding: 0 11px;
  border: 1px solid transparent;
  border-radius: 3px;
  background: transparent;
  color: var(--text-secondary);
  white-space: nowrap;
  transition: background-color var(--dur-fast) var(--ease);
}
.item.iconOnly { padding: 0 7px; }
.item:hover:not(.on) { background: var(--subtle-fill-hover); color: var(--text-primary); }
.item:active:not(.on) { background: var(--subtle-fill-pressed); }
.item.on {
  background: var(--control-fill);
  border-color: var(--elev-top);
  border-bottom-color: var(--elev-bottom);
  color: var(--text-primary);
}
.item.on::after {
  content: '';
  position: absolute;
  left: 50%;
  bottom: 2px;
  width: 16px;
  height: 3px;
  margin-left: -8px;
  border-radius: 2px;
  background: var(--accent-fill);
}
.small .item { height: 22px; padding: 0 8px; font-size: 12px; }
.small .item.iconOnly { padding: 0 5px; }
.small .item.on::after { width: 12px; margin-left: -6px; height: 2px; bottom: 1px; }
</style>
