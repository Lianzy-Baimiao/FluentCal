<script setup lang="ts">
import { ref } from 'vue';
import FIcon from './FIcon.vue';
import { useFieldName, useSplitAttrs } from './field';

defineOptions({ inheritAttrs: false });

const props = withDefaults(
  defineProps<{
    modelValue: string;
    placeholder?: string;
    icon?: string;
    clearable?: boolean;
    multiline?: boolean;
    rows?: number;
    maxlength?: number;
    width?: number | string;
    disabled?: boolean;
    label?: string;
    /** Reduced height (24px) for dense toolbars. */
    small?: boolean;
  }>(),
  { rows: 3 },
);
const emit = defineEmits<{
  'update:modelValue': [string];
  enter: [KeyboardEvent];
  escape: [KeyboardEvent];
  blur: [FocusEvent];
  focus: [FocusEvent];
}>();

const input = ref<HTMLInputElement | HTMLTextAreaElement>();
const name = useFieldName(() => props.label, () => props.placeholder);
const split = useSplitAttrs();

function onInput(e: Event) {
  emit('update:modelValue', (e.target as HTMLInputElement).value);
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') emit('escape', e);
  else if (e.key === 'Enter' && !e.isComposing && (!props.multiline || e.ctrlKey)) emit('enter', e);
}

function clear() {
  emit('update:modelValue', '');
  input.value?.focus();
}

defineExpose({
  focus: () => input.value?.focus(),
  select: () => input.value?.select(),
  el: input,
});
</script>

<template>
  <div
    class="f-text"
    :class="{ disabled, multi: multiline, small }"
    :style="{ width: typeof width === 'number' ? `${width}px` : width }"
    v-bind="split.root()"
  >
    <FIcon v-if="icon" :name="icon" :size="14" class="lead" />
    <textarea
      v-if="multiline"
      ref="input"
      :value="modelValue"
      :placeholder="placeholder"
      :rows="rows"
      :maxlength="maxlength"
      :disabled="disabled"
      v-bind="{ ...split.control(), ...name.attrs() }"
      @input="onInput"
      @keydown="onKey"
      @blur="emit('blur', $event)"
      @focus="emit('focus', $event)"
    />
    <input
      v-else
      ref="input"
      type="text"
      :value="modelValue"
      :placeholder="placeholder"
      :maxlength="maxlength"
      :disabled="disabled"
      v-bind="{ ...split.control(), ...name.attrs() }"
      spellcheck="false"
      @input="onInput"
      @keydown="onKey"
      @blur="emit('blur', $event)"
      @focus="emit('focus', $event)"
    />
    <button
      v-if="clearable && modelValue && !disabled"
      type="button"
      class="clear"
      tabindex="-1"
      aria-label="清除"
      @mousedown.prevent
      @click="clear"
    >
      <FIcon name="close" :size="10" />
    </button>
    <slot name="suffix" />
  </div>
</template>

<style scoped>
.f-text {
  position: relative;
  display: inline-flex;
  align-items: center;
  min-height: 32px;
  border-radius: var(--radius-control);
  background: var(--control-fill);
  border: 1px solid var(--control-stroke);
  border-bottom-color: var(--control-strong-stroke);
  overflow: hidden;
  transition: background-color var(--dur-fast) var(--ease);
}
.f-text::after {
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
.f-text:hover:not(.disabled) { background: var(--control-fill-hover); }
.f-text:focus-within { background: var(--control-fill-input-active); }
.f-text:focus-within::after { transform: scaleX(1); }
input, textarea {
  flex: 1;
  min-width: 0;
  border: 0;
  outline: none;
  background: transparent;
  padding: 5px 10px 6px 11px;
  font-size: 14px;
  line-height: 20px;
  color: var(--text-primary);
  box-shadow: none;
}
input:focus-visible, textarea:focus-visible { outline: none; box-shadow: none; }
textarea { resize: none; padding-top: 6px; align-self: stretch; }
.multi { align-items: stretch; }
::placeholder { color: var(--text-secondary); opacity: 1; }
.f-text:focus-within ::placeholder { color: var(--text-tertiary); }
.lead { margin-left: 10px; color: var(--text-secondary); }
.lead + input { padding-left: 8px; }
.clear {
  width: 28px;
  height: 24px;
  margin-right: 4px;
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
.clear:hover { background: var(--subtle-fill-hover); }
.small { min-height: 24px; }
.small input { padding: 1px 8px 2px; font-size: 12px; line-height: 18px; }
.disabled { background: var(--control-fill-disabled); border-color: var(--control-stroke); }
.disabled input, .disabled textarea { color: var(--text-disabled); }
.disabled ::placeholder { color: var(--text-disabled); }
</style>
