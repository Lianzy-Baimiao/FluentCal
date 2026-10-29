<script setup lang="ts">
import { ref } from 'vue';
import { anchorRect, focusables, place, pushPopup, type Anchor, type Placement } from './placement';

const props = withDefaults(
  defineProps<{
    open: boolean;
    anchor?: Anchor;
    placement?: Placement;
    offset?: number;
    /** Fixed width in px, or 'anchor' to be at least as wide as the anchor element. */
    width?: number | 'anchor';
    padding?: boolean;
    lightDismiss?: boolean;
    /** Move focus into the flyout when it opens (first focusable element, else the panel). */
    autofocus?: boolean;
    label?: string;
    role?: string;
  }>(),
  { placement: 'bottom-start', offset: 4, padding: true, lightDismiss: true, autofocus: true, role: 'dialog' },
);
const emit = defineEmits<{ 'update:open': [boolean]; opened: [HTMLElement]; closed: [] }>();

const panel = ref<HTMLElement>();
let release: (() => void) | null = null;
let ro: ResizeObserver | null = null;
let restoreFocus: HTMLElement | null = null;
let onScroll: (() => void) | null = null;

function close() {
  emit('update:open', false);
}

function position(el: HTMLElement) {
  const a = anchorRect(props.anchor);
  if (props.width === 'anchor') el.style.minWidth = `${a.width}px`;
  const p = place(a, el.offsetWidth, el.offsetHeight, props.placement, props.offset);
  el.style.left = `${p.left}px`;
  el.style.top = `${p.top}px`;
  const d = 8;
  el.style.setProperty('--fly-x', p.side === 'right' ? `-${d}px` : p.side === 'left' ? `${d}px` : '0px');
  el.style.setProperty('--fly-y', p.side === 'bottom' ? `-${d}px` : p.side === 'top' ? `${d}px` : '0px');
}

function onEnter(el: Element) {
  const node = el as HTMLElement;
  position(node);
  ro = new ResizeObserver(() => position(node));
  ro.observe(node);
  onScroll = () => position(node);
  window.addEventListener('scroll', onScroll, true);
  if (props.lightDismiss) {
    release = pushPopup({
      el: () => panel.value,
      close,
      anchor: () => (props.anchor instanceof HTMLElement ? props.anchor : null),
    });
  }
  restoreFocus = document.activeElement as HTMLElement | null;
  if (props.autofocus) {
    requestAnimationFrame(() => (focusables(node)[0] ?? node).focus({ preventScroll: true }));
  }
  emit('opened', node);
}

function cleanup() {
  release?.();
  release = null;
  ro?.disconnect();
  ro = null;
  if (onScroll) window.removeEventListener('scroll', onScroll, true);
  onScroll = null;
}

function onAfterLeave() {
  const active = document.activeElement;
  if (restoreFocus?.isConnected && (!active || active === document.body)) restoreFocus.focus({ preventScroll: true });
  restoreFocus = null;
  emit('closed');
}

defineExpose({ panel, reposition: () => panel.value && position(panel.value) });
</script>

<template>
  <Teleport to="body">
    <Transition name="f-fly" @enter="onEnter" @before-leave="cleanup" @after-leave="onAfterLeave">
      <div
        v-if="open"
        ref="panel"
        class="f-flyout"
        :class="{ pad: padding }"
        :style="{ width: typeof width === 'number' ? `${width}px` : undefined }"
        :role="role"
        :aria-label="label"
        :tabindex="role === 'presentation' ? undefined : -1"
        data-f-popup
      >
        <slot />
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.f-flyout {
  position: fixed;
  left: 0;
  top: 0;
  z-index: 1000;
  max-width: calc(100vw - 16px);
  max-height: calc(100vh - 16px);
  overflow: auto;
  color: var(--text-primary);
  background: var(--flyout-fill);
  -webkit-backdrop-filter: blur(30px) saturate(125%);
  backdrop-filter: blur(30px) saturate(125%);
  border: 1px solid var(--surface-stroke-flyout);
  border-radius: var(--radius-overlay);
  box-shadow: var(--shadow-flyout);
}
.f-flyout:focus-visible { outline: none; box-shadow: var(--shadow-flyout); }
.pad { padding: 16px; }
.f-fly-enter-active { transition: opacity var(--dur-normal) var(--ease), transform var(--dur-slow) var(--ease); }
.f-fly-leave-active { transition: opacity var(--dur-fast) linear; }
.f-fly-enter-from { opacity: 0; transform: translate(var(--fly-x, 0), var(--fly-y, -8px)); }
.f-fly-leave-to { opacity: 0; }
</style>
