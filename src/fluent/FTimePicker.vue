<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import FFlyout from './FFlyout.vue';
import FIcon from './FIcon.vue';
import FTextBox from './FTextBox.vue';
import { useSplitAttrs } from './field';

defineOptions({ inheritAttrs: false });
import { pad, parseTimeInput } from './dates';

const props = withDefaults(
  defineProps<{
    modelValue: string;
    placeholder?: string;
    step?: 15 | 30;
    width?: number | string;
    clearable?: boolean;
    disabled?: boolean;
    label?: string;
  }>(),
  { placeholder: '时间', step: 30, width: 120 },
);
const emit = defineEmits<{ 'update:modelValue': [string] }>();
const split = useSplitAttrs();

const text = ref(props.modelValue);
const open = ref(false);
const root = ref<HTMLElement>();
const list = ref<HTMLElement>();
watch(() => props.modelValue, (v) => (text.value = v));

const times = computed(() => {
  const out: string[] = [];
  for (let m = 0; m < 24 * 60; m += props.step) out.push(`${pad(Math.floor(m / 60))}:${pad(m % 60)}`);
  return out;
});

function commit() {
  const t = text.value.trim();
  if (!t) {
    if (props.clearable) emit('update:modelValue', '');
    else text.value = props.modelValue;
    return;
  }
  const v = parseTimeInput(t);
  if (v) {
    text.value = v;
    if (v !== props.modelValue) emit('update:modelValue', v);
  } else text.value = props.modelValue;
}

function onText(v: string) {
  text.value = v;
  if (v === '' && props.clearable) emit('update:modelValue', '');
}

function pick(v: string) {
  text.value = v;
  emit('update:modelValue', v);
  open.value = false;
}

function onOpened() {
  const now = new Date();
  const target = props.modelValue || `${pad(now.getHours())}:${pad(now.getMinutes())}`;
  const i = Math.max(0, times.value.findIndex((t) => t >= target));
  nextTick(() => {
    const el = list.value?.children[i] as HTMLElement | undefined;
    el?.scrollIntoView({ block: 'center' });
    el?.focus({ preventScroll: true });
  });
}

function key(e: KeyboardEvent) {
  const items = [...(list.value?.children ?? [])] as HTMLElement[];
  const i = items.indexOf(document.activeElement as HTMLElement);
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    items[Math.min(items.length - 1, Math.max(0, i + (e.key === 'ArrowDown' ? 1 : -1)))]?.focus();
  }
}
</script>

<template>
  <div ref="root" class="f-time" v-bind="split.root()">
    <FTextBox
      :model-value="text"
      :placeholder="placeholder"
      :clearable="clearable"
      :disabled="disabled"
      :width="width"
      :label="label"
      v-bind="split.forward()"
      @update:model-value="onText"
      @blur="commit"
      @enter="commit"
    >
      <template #suffix>
        <button
          type="button"
          class="clk-btn"
          :disabled="disabled"
          aria-label="选择时间"
          :aria-expanded="open"
          @mousedown.prevent
          @click="open = !open"
        >
          <FIcon name="clock" :size="14" />
        </button>
      </template>
    </FTextBox>
    <FFlyout v-model:open="open" :anchor="root" width="anchor" :padding="false" :autofocus="false" label="时间" @opened="onOpened">
      <div ref="list" class="f-times" role="listbox" @keydown="key">
        <button
          v-for="t in times"
          :key="t"
          type="button"
          class="item"
          :class="{ sel: t === modelValue }"
          role="option"
          :aria-selected="t === modelValue"
          @click="pick(t)"
        >{{ t }}</button>
      </div>
    </FFlyout>
  </div>
</template>

<style scoped>
.f-time { display: inline-flex; }
.clk-btn {
  width: 28px;
  height: 24px;
  margin-right: 3px;
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
.clk-btn:hover:not(:disabled) { background: var(--subtle-fill-hover); }
.clk-btn:disabled { color: var(--text-disabled); }
.f-times { display: flex; flex-direction: column; max-height: 264px; overflow-y: auto; padding: 2px 0; }
.item {
  position: relative;
  min-height: 32px;
  margin: 2px 4px;
  padding: 0 12px;
  border: 0;
  border-radius: var(--radius-control);
  background: transparent;
  text-align: left;
  font-variant-numeric: tabular-nums;
}
.item:hover, .item:focus-visible { background: var(--subtle-fill-hover); }
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
</style>
