<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import FCalendarView from './FCalendarView.vue';
import FFlyout from './FFlyout.vue';
import FIcon from './FIcon.vue';
import FTextBox from './FTextBox.vue';
import { useSplitAttrs } from './field';

defineOptions({ inheritAttrs: false });
import { parseDateInput, toDate } from './dates';

const props = withDefaults(
  defineProps<{
    modelValue: string;
    placeholder?: string;
    firstDay?: 0 | 1;
    lunar?: (iso: string) => string;
    markers?: (iso: string) => string | undefined;
    clearable?: boolean;
    width?: number | string;
    disabled?: boolean;
    label?: string;
  }>(),
  { placeholder: '选择日期', firstDay: 1, width: 180 },
);
const emit = defineEmits<{ 'update:modelValue': [string] }>();
const split = useSplitAttrs();

const text = ref(props.modelValue);
const open = ref(false);
const root = ref<HTMLElement>();
watch(() => props.modelValue, (v) => (text.value = v));

const weekday = computed(() => {
  const d = toDate(props.modelValue);
  return d ? `周${'日一二三四五六'[d.getDay()]}` : '';
});

function commit() {
  const t = text.value.trim();
  if (!t) {
    if (props.clearable) emit('update:modelValue', '');
    else text.value = props.modelValue;
    return;
  }
  const v = parseDateInput(t);
  if (v) {
    text.value = v;
    if (v !== props.modelValue) emit('update:modelValue', v);
  } else {
    text.value = props.modelValue;
  }
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
</script>

<template>
  <div ref="root" class="f-date" v-bind="split.root()">
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
        <span v-if="weekday" class="wd">{{ weekday }}</span>
        <button
          type="button"
          class="cal-btn"
          :disabled="disabled"
          aria-label="打开日历"
          :aria-expanded="open"
          @mousedown.prevent
          @click="open = !open"
        >
          <FIcon name="calendar" :size="14" />
        </button>
      </template>
    </FTextBox>
    <FFlyout v-model:open="open" :anchor="root" placement="bottom-start" :padding="false" label="日历">
      <FCalendarView
        :model-value="modelValue"
        :first-day="firstDay"
        :lunar="lunar"
        :markers="markers"
        @update:model-value="pick"
      />
    </FFlyout>
  </div>
</template>

<style scoped>
.f-date { display: inline-flex; }
.wd { font-size: 12px; color: var(--text-secondary); padding: 0 2px; white-space: nowrap; }
.cal-btn {
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
.cal-btn:hover:not(:disabled) { background: var(--subtle-fill-hover); }
.cal-btn:active:not(:disabled) { background: var(--subtle-fill-pressed); }
.cal-btn:disabled { color: var(--text-disabled); }
</style>
