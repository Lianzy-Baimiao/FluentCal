<script setup lang="ts">
import { computed, ref } from 'vue';
import FButton from './FButton.vue';
import FFlyout from './FFlyout.vue';
import FIcon from './FIcon.vue';
import FTextBox from './FTextBox.vue';
import { useFieldName } from './field';
import { bestTextOn, normalizeHex } from './theme';

const props = withDefaults(
  defineProps<{
    modelValue: string;
    colors: { key: string; color: string; label?: string }[];
    allowNone?: boolean;
    custom?: boolean;
    size?: number;
    label?: string;
  }>(),
  { size: 24 },
);
const emit = defineEmits<{ 'update:modelValue': [string] }>();
const name = useFieldName(() => props.label, () => '颜色');

const addBtn = ref<HTMLElement>();
const pickerOpen = ref(false);
const draft = ref('#0078D4');
const draftValid = computed(() => !!normalizeHex(draft.value));
/** A custom hex value that is not one of the preset keys/colours. */
const customValue = computed(() => {
  const v = props.modelValue;
  if (!v || props.colors.some((c) => c.key === v)) return '';
  return normalizeHex(v) ?? '';
});

function pick(v: string) {
  if (v !== props.modelValue) emit('update:modelValue', v);
}

function openPicker() {
  draft.value = customValue.value || normalizeHex(props.colors.find((c) => c.key === props.modelValue)?.color ?? '') || '#0078D4';
  pickerOpen.value = true;
}

function applyCustom() {
  const h = normalizeHex(draft.value);
  if (!h) return;
  pick(h);
  pickerOpen.value = false;
}

function key(e: KeyboardEvent) {
  const btns = [...(e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('.sw')];
  const i = btns.indexOf(document.activeElement as HTMLElement);
  const d = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
  if (d && i >= 0) {
    e.preventDefault();
    btns[(i + d + btns.length) % btns.length].focus();
  }
}
</script>

<template>
  <div class="f-swatches" :style="{ '--sw': `${size}px` }" @keydown="key">
    <div class="group" role="radiogroup" v-bind="name.attrs()">
    <button
      v-if="allowNone"
      type="button"
      class="sw none"
      :class="{ sel: !modelValue }"
      role="radio"
      :aria-checked="!modelValue"
      title="无"
      @click="pick('')"
    />
    <button
      v-for="c in colors"
      :key="c.key"
      type="button"
      class="sw"
      :class="{ sel: modelValue === c.key }"
      :style="{ background: c.color, color: bestTextOn(c.color) }"
      role="radio"
      :aria-checked="modelValue === c.key"
      :title="c.label ?? c.key"
      @click="pick(c.key)"
    >
      <FIcon v-if="modelValue === c.key" name="checkMark" :size="Math.round(size * 0.5)" />
    </button>
    <button
      v-if="customValue"
      type="button"
      class="sw sel"
      :style="{ background: customValue, color: bestTextOn(customValue) }"
      role="radio"
      aria-checked="true"
      :title="customValue"
    >
      <FIcon name="checkMark" :size="Math.round(size * 0.5)" />
    </button>
    </div>
    <button v-if="custom" ref="addBtn" type="button" class="sw add" title="自定义颜色" @click="openPicker">
      <FIcon name="add" :size="Math.round(size * 0.5)" />
    </button>
    <FFlyout v-model:open="pickerOpen" :anchor="addBtn" placement="bottom-start" :width="232" label="自定义颜色">
      <div class="picker">
        <input v-model="draft" type="color" class="native" aria-label="选择颜色" />
        <FTextBox v-model="draft" :width="120" label="颜色值" @enter="applyCustom" />
      </div>
      <div class="actions">
        <FButton variant="accent" :disabled="!draftValid" @click="applyCustom">确定</FButton>
        <FButton @click="pickerOpen = false">取消</FButton>
      </div>
    </FFlyout>
  </div>
</template>

<style scoped>
.f-swatches { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
.group { display: contents; }
.sw {
  position: relative;
  width: var(--sw);
  height: var(--sw);
  padding: 0;
  border: 1px solid var(--control-stroke);
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  transition: transform var(--dur-fast) var(--ease);
}
.sw:hover { transform: scale(1.08); }
.sw.sel { outline: 2px solid var(--text-primary); outline-offset: 2px; }
.sw.sel:focus-visible { outline-width: 3px; box-shadow: none; }
.none { background: transparent; border-color: var(--control-strong-stroke); overflow: hidden; }
.none::after {
  content: '';
  position: absolute;
  left: 50%;
  top: -10%;
  width: 1.5px;
  height: 120%;
  background: var(--critical);
  transform: rotate(45deg);
}
.add { background: transparent; border: 1px dashed var(--control-strong-stroke); color: var(--text-secondary); }
.add:hover { background: var(--subtle-fill-hover); }
.picker { display: flex; align-items: center; gap: 12px; }
.native {
  width: 48px;
  height: 32px;
  padding: 0;
  border: 1px solid var(--control-stroke);
  border-radius: var(--radius-control);
  background: none;
}
.actions { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; margin-top: 16px; }
</style>
