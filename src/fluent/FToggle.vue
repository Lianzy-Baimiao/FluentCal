<script setup lang="ts">
import { useFieldName, useSplitAttrs } from './field';

defineOptions({ inheritAttrs: false });
const props = withDefaults(
  defineProps<{
    modelValue: boolean;
    onText?: string;
    offText?: string;
    showText?: boolean;
    /** Which side of the switch the 开/关 text sits on (Windows Settings puts it left). */
    textPosition?: 'left' | 'right';
    disabled?: boolean;
    label?: string;
  }>(),
  { onText: '开', offText: '关', showText: true, textPosition: 'left' },
);
const emit = defineEmits<{ 'update:modelValue': [boolean] }>();
// The 开/关 text sits outside the switch so it never conflicts with the switch's name.
const name = useFieldName(() => props.label, () => '开关');
const split = useSplitAttrs();

function toggle() {
  if (!props.disabled) emit('update:modelValue', !props.modelValue);
}
</script>

<template>
  <span class="f-toggle" :class="{ on: modelValue, right: textPosition === 'right', disabled }" v-bind="split.root()">
    <span v-if="showText" class="txt" aria-hidden="true" @click="toggle">{{ modelValue ? onText : offText }}</span>
    <button
      type="button"
      class="sw"
      role="switch"
      :aria-checked="modelValue"
      :disabled="disabled"
      v-bind="{ ...split.control(), ...name.attrs() }"
      @click="toggle"
    >
      <span class="track"><span class="knob" /></span>
    </button>
  </span>
</template>

<style scoped>
.f-toggle {
  display: inline-flex;
  align-items: center;
  gap: 12px;
  min-height: 32px;
  color: var(--text-primary);
}
.f-toggle.right { flex-direction: row-reverse; }
.txt { min-width: 2em; text-align: right; }
.f-toggle.right .txt { text-align: left; }
.sw {
  padding: 0;
  border: 0;
  background: none;
  border-radius: 10px;
  display: inline-flex;
}

.track {
  position: relative;
  width: 40px;
  height: 20px;
  border-radius: 10px;
  background: var(--control-alt-fill);
  border: 1px solid var(--control-strong-stroke);
  transition: background-color var(--dur-fast) var(--ease), border-color var(--dur-fast) var(--ease);
  flex: none;
}
.knob {
  position: absolute;
  top: 50%;
  left: 3px;
  width: 12px;
  height: 12px;
  margin-top: -6px;
  border-radius: 6px;
  background: var(--text-secondary);
  transition: left var(--dur-normal) var(--ease), width var(--dur-fast) var(--ease),
    height var(--dur-fast) var(--ease), margin var(--dur-fast) var(--ease), background-color var(--dur-fast);
}
.f-toggle:not(.disabled):hover .track { background: var(--control-alt-fill-hover); }
.f-toggle:not(.disabled):hover .knob { width: 14px; height: 14px; margin-top: -7px; left: 2px; }
.f-toggle:not(.disabled):active .track { background: var(--control-alt-fill-pressed); }
.f-toggle:not(.disabled):active .knob { width: 17px; height: 14px; margin-top: -7px; left: 2px; }

.on .track { background: var(--accent-fill); border-color: transparent; }
.on .knob { left: 23px; background: var(--text-on-accent); }
.on:not(.disabled):hover .track { background: var(--accent-fill-hover); }
.on:not(.disabled):hover .knob { left: 22px; }
.on:not(.disabled):active .track { background: var(--accent-fill-pressed); }
.on:not(.disabled):active .knob { left: 19px; }

.disabled { color: var(--text-disabled); }
.disabled .track { border-color: var(--control-strong-stroke-disabled); }
.disabled .knob { background: var(--text-disabled); }
.on.disabled .track { background: var(--accent-fill-disabled); border-color: transparent; }
.on.disabled .knob { background: var(--text-on-accent-disabled); }
</style>
