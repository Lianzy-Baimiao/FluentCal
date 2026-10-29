<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import FIcon from './FIcon.vue';
import type { MenuItem, MenuPlacement } from './menu';
import { uid } from './field';
import { anchorRect, place, pushPopup, type Anchor } from './placement';
import { bestTextOn } from './theme';

const props = defineProps<{
  items: MenuItem[];
  anchor: Anchor;
  placement: MenuPlacement;
  depth: number;
  /** Opened from the keyboard: highlight the first item. */
  keyboard?: boolean;
}>();
const emit = defineEmits<{ 'close-all': []; 'close-self': []; 'pointer-in': [] }>();

const panel = ref<HTMLElement>();
const hi = ref(-1);
const sub = ref(-1);
const subAnchor = ref<DOMRect | null>(null);
const subKeyboard = ref(false);
const swatchHi = ref(-1);
const side = ref('bottom');
let release: (() => void) | null = null;
let timer = 0;
const pid = uid('fm');
function activeId(): string | undefined {
  const it = props.items[hi.value];
  if (!it) return undefined;
  if (it.swatches) return swatchHi.value >= 0 ? `${pid}-${hi.value}-${swatchHi.value}` : undefined;
  return `${pid}-${hi.value}`;
}

const hasCheck = computed(() => props.items.some((i) => i.checked !== undefined));
const hasIcon = computed(() => props.items.some((i) => i.icon));
const enabled = (i: number) => {
  const it = props.items[i];
  return !!it && !it.separator && !it.disabled;
};
const isNone = (c: string) => !c || c === 'transparent';

function position() {
  const el = panel.value;
  if (!el) return;
  const a = anchorRect(props.anchor);
  const w = el.offsetWidth, h = el.offsetHeight;
  const sub = props.depth > 0 || props.placement === 'right-start';
  const p = sub ? place({ ...a, top: a.top - 5 }, w, h, 'right-start', -2) : place(a, w, h, props.placement, a.width ? 4 : 0);
  // Context menus at a point open to the left of the cursor when there is no room on the right.
  if (!sub && a.width === 0 && p.left < a.left) p.left = Math.max(8, a.left - w);
  el.style.left = `${p.left}px`;
  el.style.top = `${p.top}px`;
  side.value = p.side;
}

function firstEnabled(from = -1, dir: 1 | -1 = 1): number {
  const n = props.items.length;
  for (let k = 1; k <= n; k++) {
    const j = (from + dir * k + n * 2) % n;
    if (enabled(j)) return j;
  }
  return -1;
}

onMounted(() => {
  position();
  const anchorEl = props.depth === 0 && props.anchor instanceof HTMLElement ? props.anchor : null;
  release = pushPopup({
    el: () => panel.value,
    close: () => (props.depth === 0 ? emit('close-all') : emit('close-self')),
    anchor: anchorEl ? () => anchorEl : undefined,
  });
  if (props.keyboard) hi.value = firstEnabled();
  nextTick(() => panel.value?.focus({ preventScroll: true }));
});
onBeforeUnmount(() => {
  release?.();
  clearTimeout(timer);
});
watch(sub, (v) => v < 0 && panel.value?.focus({ preventScroll: true }));

function openSub(i: number, keyboard: boolean) {
  const el = panel.value?.querySelector<HTMLElement>(`[data-i="${i}"]`);
  if (!el) return;
  subAnchor.value = el.getBoundingClientRect();
  subKeyboard.value = keyboard;
  sub.value = i;
}

function enter(i: number) {
  clearTimeout(timer);
  if (!enabled(i)) return;
  hi.value = i;
  swatchHi.value = -1;
  const it = props.items[i];
  if (it.children?.length) {
    if (sub.value !== i) timer = window.setTimeout(() => openSub(i, false), 250);
  } else if (sub.value >= 0) {
    timer = window.setTimeout(() => (sub.value = -1), 300);
  }
}

function leave() {
  if (sub.value < 0) hi.value = -1;
}

function subPointerIn() {
  clearTimeout(timer);
  hi.value = sub.value;
}

function closeSub() {
  sub.value = -1;
}

function activate(i: number, keyboard = false) {
  const it = props.items[i];
  if (!it || it.separator || it.disabled || it.swatches) return;
  if (it.children?.length) {
    openSub(i, keyboard);
    return;
  }
  emit('close-all');
  it.action?.();
}

function swatch(it: MenuItem, key: string) {
  emit('close-all');
  it.onSwatch?.(key);
}

function key(e: KeyboardEvent) {
  const it = props.items[hi.value];
  const move = (dir: 1 | -1) => {
    const j = firstEnabled(hi.value, dir);
    if (j >= 0) {
      hi.value = j;
      swatchHi.value = props.items[j].swatches ? 0 : -1;
    }
  };
  switch (e.key) {
    case 'ArrowDown': e.preventDefault(); move(1); break;
    case 'ArrowUp': e.preventDefault(); move(-1); break;
    case 'Home': e.preventDefault(); hi.value = -1; move(1); break;
    case 'End': e.preventDefault(); hi.value = 0; move(-1); break;
    case 'ArrowRight':
      e.preventDefault();
      if (it?.swatches) swatchHi.value = Math.min(it.swatches.length - 1, swatchHi.value + 1);
      else if (it?.children?.length) openSub(hi.value, true);
      break;
    case 'ArrowLeft':
      e.preventDefault();
      if (it?.swatches) swatchHi.value = Math.max(0, swatchHi.value - 1);
      else if (props.depth > 0) emit('close-self');
      break;
    case 'Enter':
    case ' ':
      e.preventDefault();
      if (it?.swatches && swatchHi.value >= 0) swatch(it, it.swatches[swatchHi.value].key);
      else if (hi.value >= 0) activate(hi.value, true);
      break;
    case 'Tab':
      e.preventDefault();
      emit('close-all');
      break;
  }
}
</script>

<template>
  <div
    ref="panel"
    class="f-menu"
    :data-side="side"
    role="menu"
    tabindex="-1"
    :aria-activedescendant="activeId()"
    data-f-popup
    @keydown="key"
    @pointerenter="emit('pointer-in')"
    @pointerleave="leave"
    @contextmenu.prevent
  >
    <template v-for="(it, i) in items" :key="it.key ?? i">
      <div v-if="it.separator" class="sep" role="separator" />
      <div v-else-if="it.swatches" class="swatch-row" role="group" :aria-label="it.label" :data-i="i" @pointerenter="enter(i)">
        <span v-if="it.label" class="sw-label">{{ it.label }}</span>
        <div class="sws">
          <button
            v-for="(s, k) in it.swatches"
            :id="`${pid}-${i}-${k}`"
            :key="s.key"
            type="button"
            role="menuitemradio"
            :aria-checked="!!s.selected"
            class="sw"
            :class="{ sel: s.selected, kb: i === hi && k === swatchHi, none: isNone(s.color) }"
            :style="isNone(s.color) ? undefined : { background: s.color, color: bestTextOn(s.color) }"
            :title="s.title"
            :aria-label="s.title ?? s.key"
            tabindex="-1"
            @click="swatch(it, s.key)"
          >
            <FIcon v-if="s.selected && !isNone(s.color)" name="checkMark" :size="10" />
          </button>
        </div>
      </div>
      <div
        v-else
        :id="`${pid}-${i}`"
        class="item"
        :class="{ hi: i === hi, open: i === sub, disabled: it.disabled, danger: it.danger }"
        :data-i="i"
        :role="it.checked !== undefined ? (it.radio ? 'menuitemradio' : 'menuitemcheckbox') : 'menuitem'"
        :aria-checked="it.checked"
        :aria-disabled="it.disabled"
        :aria-haspopup="it.children?.length ? 'menu' : undefined"
        :aria-expanded="it.children?.length ? i === sub : undefined"
        @pointerenter="enter(i)"
        @click="activate(i)"
      >
        <span v-if="hasCheck" class="chk">
          <FIcon v-if="it.checked" :name="it.radio ? 'statusCircleInner' : 'checkMark'" :size="it.radio ? 8 : 12" />
        </span>
        <FIcon v-if="hasIcon" :name="it.icon ?? ''" :size="16" />
        <span class="lbl">{{ it.label }}</span>
        <span v-if="it.shortcut" class="sc">{{ it.shortcut }}</span>
        <FIcon v-if="it.children?.length" name="chevronRight" :size="12" class="subi" />
      </div>
    </template>
  </div>
  <MenuPanel
    v-if="sub >= 0 && items[sub]?.children"
    :items="items[sub].children!"
    :anchor="subAnchor"
    placement="right-start"
    :depth="depth + 1"
    :keyboard="subKeyboard"
    @close-all="emit('close-all')"
    @close-self="closeSub"
    @pointer-in="subPointerIn"
  />
</template>

<style scoped>
.f-menu {
  position: fixed;
  left: 0;
  top: 0;
  z-index: 1100;
  min-width: 160px;
  max-width: min(360px, calc(100vw - 16px));
  max-height: calc(100vh - 16px);
  overflow-y: auto;
  padding: 2px 0;
  background: var(--flyout-fill);
  -webkit-backdrop-filter: blur(30px) saturate(125%);
  backdrop-filter: blur(30px) saturate(125%);
  border: 1px solid var(--surface-stroke-flyout);
  border-radius: var(--radius-overlay);
  box-shadow: var(--shadow-flyout);
  color: var(--text-primary);
  animation: f-menu-in var(--dur-normal) var(--ease);
}
.f-menu[data-side='top'] { --mdy: 6px; }
.f-menu[data-side='left'] { --mdx: 6px; --mdy: 0px; }
.f-menu[data-side='right'] { --mdx: -6px; --mdy: 0px; }
.f-menu:focus-visible { outline: none; box-shadow: var(--shadow-flyout); }
@keyframes f-menu-in {
  from { opacity: 0; transform: translate(var(--mdx, 0px), var(--mdy, -6px)); }
}
.item {
  display: flex;
  align-items: center;
  gap: 12px;
  height: 32px;
  margin: 2px 4px;
  padding: 0 12px;
  border-radius: var(--radius-control);
  white-space: nowrap;
}
.item.hi, .item.open { background: var(--subtle-fill-hover); }
.item:active:not(.disabled) { background: var(--subtle-fill-pressed); color: var(--text-secondary); }
.item.disabled { color: var(--text-disabled); }
.item.danger:not(.disabled) { color: var(--critical); }
.chk { width: 16px; display: inline-flex; justify-content: center; flex: none; }
.lbl { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; }
.sc { margin-left: 24px; font-size: 12px; color: var(--text-secondary); }
.item.disabled .sc { color: var(--text-disabled); }
.subi { margin-left: 8px; color: var(--text-secondary); }
.sep { height: 1px; margin: 3px 0; background: var(--divider); }
.swatch-row { margin: 2px 4px; padding: 6px 12px 8px; }
.sw-label { display: block; margin-bottom: 6px; font-size: 12px; line-height: 16px; color: var(--text-secondary); }
.sws { display: flex; flex-wrap: wrap; gap: 6px; max-width: 232px; }
.sw {
  position: relative;
  width: 20px;
  height: 20px;
  padding: 0;
  border: 1px solid var(--control-stroke);
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  transition: transform var(--dur-fast) var(--ease);
}
.sw:hover { transform: scale(1.12); }
.sw.sel, .sw.kb { outline: 2px solid var(--text-primary); outline-offset: 1px; }
.sw.none { background: transparent; border-color: var(--control-strong-stroke); }
.sw.none::after {
  content: '';
  position: absolute;
  left: 50%;
  top: -10%;
  width: 1.5px;
  height: 120%;
  background: var(--critical);
  transform: rotate(45deg);
}
</style>
