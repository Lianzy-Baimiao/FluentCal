<script setup lang="ts">
import { computed, nextTick, ref } from 'vue';
import FFlyout from './FFlyout.vue';
import FIcon from './FIcon.vue';
import { useFieldName, useSplitAttrs } from './field';

defineOptions({ inheritAttrs: false });

type V = string | number;
interface Opt { value: V; label: string; icon?: string; disabled?: boolean }

const props = withDefaults(
  defineProps<{
    modelValue: V | null | undefined;
    options: Opt[];
    placeholder?: string;
    width?: number | string;
    disabled?: boolean;
    label?: string;
  }>(),
  { placeholder: '请选择' },
);
const emit = defineEmits<{ 'update:modelValue': [V] }>();
const name = useFieldName(() => props.label, () => props.placeholder);
const split = useSplitAttrs();

const field = ref<HTMLButtonElement>();
const list = ref<HTMLElement>();
const open = ref(false);
const hi = ref(-1);
const uid = `fsel-${Math.random().toString(36).slice(2, 8)}`;
const selectedIndex = computed(() => props.options.findIndex((o) => o.value === props.modelValue));
const selected = computed(() => props.options[selectedIndex.value]);

function show() {
  if (props.disabled) return;
  hi.value = selectedIndex.value >= 0 ? selectedIndex.value : props.options.findIndex((o) => !o.disabled);
  open.value = true;
}

function choose(i: number) {
  const o = props.options[i];
  if (!o || o.disabled) return;
  if (o.value !== props.modelValue) emit('update:modelValue', o.value);
  open.value = false;
  field.value?.focus();
}

function next(from: number, dir: 1 | -1): number {
  const n = props.options.length;
  for (let k = 1; k <= n; k++) {
    const i = from + dir * k;
    if (i < 0 || i >= n) break;
    if (!props.options[i].disabled) return i;
  }
  return from;
}

function scrollHi() {
  nextTick(() => list.value?.querySelector(`#${uid}-${hi.value}`)?.scrollIntoView({ block: 'nearest' }));
}

function fieldKey(e: KeyboardEvent) {
  if (['Enter', ' ', 'F4'].includes(e.key) || (e.altKey && e.key === 'ArrowDown')) {
    e.preventDefault();
    show();
  } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    const i = next(selectedIndex.value, e.key === 'ArrowDown' ? 1 : -1);
    if (i !== selectedIndex.value && i >= 0) emit('update:modelValue', props.options[i].value);
  }
}

function listKey(e: KeyboardEvent) {
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    hi.value = next(hi.value, e.key === 'ArrowDown' ? 1 : -1);
    scrollHi();
  } else if (e.key === 'Home' || e.key === 'End') {
    e.preventDefault();
    hi.value = e.key === 'Home' ? next(-1, 1) : next(props.options.length, -1);
    scrollHi();
  } else if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault();
    choose(hi.value);
  } else if (e.key === 'Tab') {
    open.value = false;
  } else if (e.key.length === 1) {
    const k = e.key.toLowerCase();
    const i = props.options.findIndex((o, j) => j > hi.value && !o.disabled && o.label.toLowerCase().startsWith(k));
    const j = i >= 0 ? i : props.options.findIndex((o) => !o.disabled && o.label.toLowerCase().startsWith(k));
    if (j >= 0) { hi.value = j; scrollHi(); }
  }
}

function onOpened() {
  list.value?.focus({ preventScroll: true });
  scrollHi();
}
</script>

<template>
  <button
    ref="field"
    type="button"
    class="f-select"
    :class="{ open, disabled }"
    :style="{ width: typeof width === 'number' ? `${width}px` : width }"
    :disabled="disabled"
    role="combobox"
    aria-haspopup="listbox"
    :aria-expanded="open"
    :aria-controls="uid"
    v-bind="{ ...split.root(), ...split.control(), ...name.attrs() }"
    @click="open ? (open = false) : show()"
    @keydown="fieldKey"
  >
    <FIcon v-if="selected?.icon" :name="selected.icon" :size="16" />
    <span class="value" :class="{ ph: !selected }">{{ selected?.label ?? placeholder }}</span>
    <FIcon name="chevronDown" :size="12" class="chev" />
  </button>
  <FFlyout
    v-model:open="open"
    :anchor="field"
    width="anchor"
    :padding="false"
    :autofocus="false"
    role="presentation"
    @opened="onOpened"
  >
    <div
      :id="uid"
      ref="list"
      class="f-list"
      role="listbox"
      tabindex="-1"
      :aria-label="name.text()"
      :aria-activedescendant="hi >= 0 ? `${uid}-${hi}` : undefined"
      @keydown="listKey"
    >
      <div
        v-for="(o, i) in options"
        :id="`${uid}-${i}`"
        :key="String(o.value)"
        class="item"
        :class="{ sel: i === selectedIndex, hi: i === hi, disabled: o.disabled }"
        role="option"
        :aria-selected="i === selectedIndex"
        :aria-disabled="o.disabled"
        @pointerenter="!o.disabled && (hi = i)"
        @click="choose(i)"
      >
        <FIcon v-if="o.icon" :name="o.icon" :size="16" />
        <span>{{ o.label }}</span>
      </div>
    </div>
  </FFlyout>
</template>

<style scoped>
.f-select {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 120px;
  height: 32px;
  padding: 0 11px 0 11px;
  border-radius: var(--radius-control);
  background: var(--control-fill);
  border: 1px solid var(--elev-top);
  border-bottom-color: var(--elev-bottom);
  color: var(--text-primary);
  text-align: left;
  transition: background-color var(--dur-fast) var(--ease);
}
.f-select:hover:not(:disabled) { background: var(--control-fill-hover); }
.f-select:active:not(:disabled), .f-select.open { background: var(--control-fill-pressed); }
.value { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ph { color: var(--text-secondary); }
.chev { color: var(--text-secondary); margin-left: 4px; }
.f-select:disabled { background: var(--control-fill-disabled); border-color: var(--control-stroke); color: var(--text-disabled); }
.f-select:disabled .chev, .f-select:disabled .ph { color: var(--text-disabled); }

.f-list { padding: 2px 0; max-height: 320px; overflow-y: auto; outline: none; }
.item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 32px;
  margin: 2px 4px;
  padding: 0 12px;
  border-radius: var(--radius-control);
  white-space: nowrap;
}
.item.hi { background: var(--subtle-fill-hover); }
.item:active:not(.disabled) { background: var(--subtle-fill-pressed); color: var(--text-secondary); }
.item.sel { background: var(--subtle-fill-hover); }
.item.sel::before {
  content: '';
  position: absolute;
  left: 0;
  top: 50%;
  width: 3px;
  height: 16px;
  margin-top: -8px;
  border-radius: 2px;
  background: var(--accent-fill);
}
.item.disabled { color: var(--text-disabled); }
</style>
