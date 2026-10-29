<script setup lang="ts">
import { computed, ref } from 'vue';
import FIcon from './FIcon.vue';
import { uid, useFieldName, useSplitAttrs } from './field';
import { modsOf, shortcutFromEvent, shortcutLabels } from './shortcut';

defineOptions({ inheritAttrs: false });
const props = withDefaults(
  defineProps<{ modelValue: string; placeholder?: string; disabled?: boolean; label?: string }>(),
  { placeholder: '未设置' },
);
const emit = defineEmits<{ 'update:modelValue': [string] }>();

const recording = ref(false);
const partial = ref<string[]>([]);
const keys = computed(() => (recording.value ? partial.value : shortcutLabels(props.modelValue)));
// The accessible name = field title + the visible keys, so it always contains the visible text.
const name = useFieldName(() => props.label);
const split = useSplitAttrs();
const ownId = uid('fsk');
const keysId = uid('fsk');
const hintId = uid('fsk');
const ownText = () => name.text() ?? '快捷键';
/** aria-labelledby: caller's ids, the settings card title, or our own hidden label. */
function labelledBy(): string {
  const a = name.attrs();
  const base = (a['aria-labelledby'] as string | undefined) ?? (a['aria-label'] || !name.ctx ? ownId : name.ctx.titleId);
  return `${base} ${keysId}`;
}
const needOwnLabel = () => {
  const a = name.attrs();
  return !a['aria-labelledby'] || !!a['aria-label'];
};
function describedBy(): string | undefined {
  const d = [recording.value ? hintId : '', (name.attrs()['aria-describedby'] as string | undefined) ?? ''].filter(Boolean).join(' ');
  return d || undefined;
}

function start() {
  if (props.disabled) return;
  recording.value = true;
  partial.value = [];
}

function stop() {
  recording.value = false;
  partial.value = [];
}

function key(e: KeyboardEvent) {
  if (!recording.value) {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); start(); }
    else if (e.key === 'Backspace' || e.key === 'Delete') { e.preventDefault(); emit('update:modelValue', ''); }
    return;
  }
  e.preventDefault();
  e.stopPropagation();
  const mods = modsOf(e);
  if (!mods.length && e.key === 'Escape') return stop();
  if (!mods.length && (e.key === 'Backspace' || e.key === 'Delete')) {
    emit('update:modelValue', '');
    return stop();
  }
  const s = shortcutFromEvent(e);
  if (s) {
    emit('update:modelValue', s);
    stop();
  } else {
    partial.value = shortcutLabels(mods.join('+'));
  }
}

function keyup(e: KeyboardEvent) {
  if (recording.value) partial.value = shortcutLabels(modsOf(e).join('+'));
}
</script>

<template>
  <button
    type="button"
    class="f-short"
    :class="{ rec: recording, disabled }"
    :disabled="disabled"
    v-bind="{ ...split.root(), ...split.control() }"
    :aria-labelledby="labelledBy()"
    :aria-describedby="describedBy()"
    @click="recording ? stop() : start()"
    @keydown="key"
    @keyup="keyup"
    @blur="stop"
  >
    <span v-if="needOwnLabel()" :id="ownId" class="visually-hidden">{{ ownText() }}</span>
    <span :id="keysId" class="keys">
      <template v-if="keys.length">
        <template v-for="(k, i) in keys" :key="i">
          <!-- spaces separate the keys for name matching; whitespace in a flex box is not rendered -->
          <template v-if="i > 0">{{ ' ' }}<span class="visually-hidden">+</span>{{ ' ' }}</template>
          <kbd>{{ k }}</kbd>
        </template>
      </template>
      <span v-else class="ph">{{ recording ? '按下快捷键…' : placeholder }}</span>
    </span>
    <span v-if="recording" :id="hintId" class="visually-hidden">按下新的快捷键，Esc 取消，Backspace 清除</span>
    <FIcon :name="recording ? 'keyboard' : 'edit'" :size="14" class="edit" />
  </button>
</template>

<style scoped>
.f-short {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  min-width: 160px;
  height: 32px;
  padding: 0 8px 0 4px;
  border-radius: var(--radius-control);
  background: var(--control-fill);
  border: 1px solid var(--elev-top);
  border-bottom-color: var(--elev-bottom);
  color: var(--text-primary);
}
.f-short:hover:not(:disabled) { background: var(--control-fill-hover); }
.rec { border: 1px dashed var(--accent-fill); background: var(--control-fill-input-active); }
.keys { display: inline-flex; align-items: center; gap: 4px; }
kbd {
  min-width: 24px;
  height: 24px;
  padding: 0 7px;
  border-radius: var(--radius-control);
  background: var(--accent-fill);
  color: var(--text-on-accent);
  font: 600 12px/24px var(--font-ui);
  text-align: center;
  box-shadow: 0 1px 0 var(--accent-elev-bottom);
}
.rec kbd { background: var(--control-solid-fill); color: var(--text-primary); box-shadow: 0 0 0 1px var(--control-stroke); }
.ph { padding: 0 6px; color: var(--text-secondary); }
.edit { margin-left: auto; padding-left: 8px; color: var(--text-secondary); }
.f-short:disabled { background: var(--control-fill-disabled); color: var(--text-disabled); }
.f-short:disabled kbd { background: var(--accent-fill-disabled); }
</style>
