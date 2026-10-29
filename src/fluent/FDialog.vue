<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue';
import FButton from './FButton.vue';
import { provideField } from './field';
import { focusables, pushPopup } from './placement';

const props = withDefaults(
  defineProps<{
    open: boolean;
    title?: string;
    primaryText?: string;
    secondaryText?: string;
    closeText?: string;
    primaryDisabled?: boolean;
    danger?: boolean;
    width?: number;
  }>(),
  { closeText: '取消' },
);
const emit = defineEmits<{ 'update:open': [boolean]; primary: []; secondary: []; close: [] }>();

const overlay = ref<HTMLElement>();
const box = ref<HTMLElement>();
const { titleId } = provideField(() => props.title ?? '', () => false);
let release: (() => void) | null = null;
let restore: HTMLElement | null = null;

function close() {
  emit('close');
  emit('update:open', false);
}
function primary() {
  if (props.primaryDisabled) return;
  emit('primary');
}
function secondary() {
  emit('secondary');
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      restore = document.activeElement as HTMLElement | null;
      // The whole overlay counts as "inside", so clicks on the smoke don't dismiss.
      release = pushPopup({ el: () => overlay.value, close });
      nextTick(() => {
        const f = box.value ? focusables(box.value) : [];
        const body = f.find((el) => !el.closest('.foot'));
        (body ?? f.find((el) => el.classList.contains('v-accent')) ?? f[0] ?? box.value)?.focus();
      });
    } else {
      release?.();
      release = null;
      if (restore?.isConnected) restore.focus({ preventScroll: true });
      restore = null;
    }
  },
  { immediate: true },
);
onBeforeUnmount(() => release?.());

function key(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.isComposing) {
    const t = e.target as HTMLElement;
    if (t.tagName === 'TEXTAREA' || t.tagName === 'BUTTON') return;
    if (props.primaryText) {
      e.preventDefault();
      primary();
    }
  } else if (e.key === 'Tab' && box.value) {
    const f = focusables(box.value);
    if (!f.length) return;
    const first = f[0], last = f[f.length - 1];
    if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
    else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition name="f-dlg">
      <div v-if="open" ref="overlay" class="f-dlg-overlay" data-f-popup @keydown="key">
        <div
          ref="box"
          class="f-dlg"
          role="dialog"
          aria-modal="true"
          :aria-labelledby="title ? titleId : undefined"
          tabindex="-1"
          :style="width ? { width: `${width}px` } : undefined"
        >
          <div class="body">
            <h2 v-if="title" :id="titleId" class="title">{{ title }}</h2>
            <div class="content"><slot /></div>
          </div>
          <div class="foot">
            <FButton v-if="primaryText" variant="accent" :class="{ danger }" :disabled="primaryDisabled" @click="primary">
              {{ primaryText }}
            </FButton>
            <FButton v-if="secondaryText" @click="secondary">{{ secondaryText }}</FButton>
            <FButton v-if="closeText" :variant="primaryText ? 'standard' : 'accent'" @click="close">{{ closeText }}</FButton>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.f-dlg-overlay {
  position: fixed;
  inset: 0;
  z-index: 900;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  background: var(--smoke);
}
.f-dlg {
  display: flex;
  flex-direction: column;
  min-width: min(320px, 100%);
  max-width: min(548px, 100%);
  max-height: calc(100vh - 32px);
  border-radius: var(--radius-overlay);
  border: 1px solid var(--surface-stroke);
  background: var(--solid-base);
  box-shadow: var(--shadow-dialog);
  color: var(--text-primary);
  overflow: hidden;
}
.f-dlg:focus-visible { outline: none; }
.body { padding: 24px; background: var(--layer-fill-alt); overflow-y: auto; min-height: 0; }
.title { margin: 0 0 12px; font-family: var(--font-display); font-size: 20px; line-height: 28px; font-weight: 600; }
.content { font-size: 14px; line-height: 20px; }
.foot {
  display: grid;
  grid-auto-flow: column;
  grid-auto-columns: 1fr;
  gap: 8px;
  padding: 24px;
  border-top: 1px solid var(--card-stroke);
  background: var(--solid-base);
}
.foot :deep(.f-btn) { width: 100%; }
.foot :deep(.f-btn.danger) { background: var(--critical); color: var(--text-inverse); border-color: transparent; }
.foot :deep(.f-btn.danger:hover) { background: color-mix(in srgb, var(--critical) 90%, transparent); }
.f-dlg-enter-active { transition: opacity var(--dur-fast) linear; }
.f-dlg-enter-active .f-dlg { transition: transform var(--dur-normal) var(--ease), opacity var(--dur-normal) var(--ease); }
.f-dlg-leave-active { transition: opacity var(--dur-fast) linear; }
.f-dlg-enter-from { opacity: 0; }
.f-dlg-enter-from .f-dlg { transform: scale(1.05); opacity: 0; }
.f-dlg-leave-to { opacity: 0; }
</style>
