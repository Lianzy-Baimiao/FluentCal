<script setup lang="ts">
import { computed } from 'vue';
import { useFieldName } from './field';
import { bestTextOn, normalizeHex } from './theme';

const props = withDefaults(
  defineProps<{
    modelValue: boolean;
    label?: string;
    indeterminate?: boolean;
    disabled?: boolean;
    /** Round variant, used for task check circles. */
    round?: boolean;
    /** Box size in px (default 20; task lists use 16). */
    size?: number;
    /** Accessible name when there is no visible label. */
    ariaLabel?: string;
    /** Custom checked colour (e.g. a task colour). */
    color?: string;
  }>(),
  { size: 20 },
);
const emit = defineEmits<{ 'update:modelValue': [boolean] }>();

const state = computed(() => (props.indeterminate ? 'mixed' : props.modelValue ? 'on' : 'off'));
const onColor = computed(() => (props.color && normalizeHex(props.color) ? bestTextOn(props.color) : undefined));
const glyph = computed(() => (props.indeterminate ? '\uE73C' : '\uE73E'));
// With a visible label the name comes from the content; otherwise from aria-label or context.
const name = useFieldName(() => props.ariaLabel);
const nameAttrs = () => {
  const a = name.attrs();
  return props.label ? { 'aria-describedby': a['aria-describedby'] } : a;
};

function toggle() {
  if (!props.disabled) emit('update:modelValue', !props.modelValue);
}
</script>

<template>
  <button
    type="button"
    class="f-check"
    :class="[state, { round, disabled, labelled: !!label }]"
    :style="{ '--sz': `${size}px`, '--ck': color, '--ckt': onColor }"
    role="checkbox"
    :aria-checked="indeterminate ? 'mixed' : modelValue"
    :disabled="disabled"
    v-bind="nameAttrs()"
    @click.stop="toggle"
  >
    <span class="box" aria-hidden="true">
      <span v-if="state !== 'off'" class="f-icon glyph">{{ glyph }}</span>
    </span>
    <span v-if="label" class="lbl">{{ label }}</span>
  </button>
</template>

<style scoped>
.f-check {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 0;
  border: 0;
  background: none;
  color: var(--text-primary);
  text-align: left;
  border-radius: var(--radius-control);
}
.f-check.labelled { min-height: 32px; }
.f-check.round { border-radius: 50%; }
.f-check.round.labelled { border-radius: var(--radius-control); }
.box {
  width: var(--sz);
  height: var(--sz);
  border-radius: var(--radius-control);
  border: 1px solid var(--control-strong-stroke);
  background: var(--control-alt-fill);
  color: var(--ckt, var(--text-on-accent));
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  transition: background-color var(--dur-fast) var(--ease), border-color var(--dur-fast);
}
.round .box { border-radius: 50%; }
.glyph { font-size: calc(var(--sz) * 0.6); }
.f-check:not(.disabled):hover .box { background: var(--control-alt-fill-hover); }
.f-check:not(.disabled):active .box { background: var(--control-alt-fill-pressed); border-color: var(--control-strong-stroke-disabled); }
.on .box, .mixed .box { background: var(--ck, var(--accent-fill)); border-color: var(--ck, var(--accent-fill)); }
.f-check.on:not(.disabled):hover .box, .f-check.mixed:not(.disabled):hover .box { background: var(--ck, var(--accent-fill-hover)); }
.on:not(.disabled):active .box, .mixed:not(.disabled):active .box { background: var(--accent-fill-pressed); }
.round.off:not(.disabled):hover .box::after {
  content: '\E73E';
  font-family: var(--font-icon);
  font-size: calc(var(--sz) * 0.6);
  color: var(--text-secondary);
}
.disabled { color: var(--text-disabled); }
.disabled .box { border-color: var(--control-strong-stroke-disabled); }
.disabled.on .box, .disabled.mixed .box { background: var(--accent-fill-disabled); border-color: transparent; color: var(--text-on-accent-disabled); }
</style>
