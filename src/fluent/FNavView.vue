<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import FIcon from './FIcon.vue';

interface Item { key: string; label: string; icon: string }
const props = defineProps<{
  items: Item[];
  footerItems?: Item[];
  modelValue: string;
  title?: string;
  /** Page heading; defaults to the selected item's label. Pass '' to hide. */
  pageTitle?: string;
}>();
const emit = defineEmits<{ 'update:modelValue': [string] }>();

const pane = ref<HTMLElement>();
const pill = ref({ top: 0, visible: false });
const all = computed(() => [...props.items, ...(props.footerItems ?? [])]);
const current = computed(() => all.value.find((i) => i.key === props.modelValue));
const heading = computed(() => (props.pageTitle !== undefined ? props.pageTitle : current.value?.label ?? ''));

function placePill() {
  const el = pane.value?.querySelector<HTMLElement>(`[data-key="${CSS.escape(props.modelValue)}"]`);
  if (!el || !pane.value) {
    pill.value = { top: 0, visible: false };
    return;
  }
  const top = el.getBoundingClientRect().top - pane.value.getBoundingClientRect().top + pane.value.scrollTop;
  pill.value = { top: top + el.offsetHeight / 2 - 8, visible: true };
}

watch(() => props.modelValue, () => nextTick(placePill));
let ro: ResizeObserver | null = null;
onMounted(() => {
  nextTick(placePill);
  ro = new ResizeObserver(placePill);
  if (pane.value) ro.observe(pane.value);
});
onBeforeUnmount(() => ro?.disconnect());

function key(e: KeyboardEvent) {
  const btns = [...(pane.value?.querySelectorAll<HTMLElement>('.nav-item') ?? [])];
  const i = btns.indexOf(document.activeElement as HTMLElement);
  if (i < 0) return;
  let j = -1;
  if (e.key === 'ArrowDown') j = Math.min(btns.length - 1, i + 1);
  else if (e.key === 'ArrowUp') j = Math.max(0, i - 1);
  else if (e.key === 'Home') j = 0;
  else if (e.key === 'End') j = btns.length - 1;
  if (j >= 0) {
    e.preventDefault();
    btns[j].focus();
  }
}
</script>

<template>
  <div class="f-nav">
    <div class="layout">
    <nav ref="pane" class="pane" :aria-label="title ?? '导航'" @keydown="key">
      <div v-if="title || $slots.paneHeader" class="pane-title">
        <slot name="paneHeader">{{ title }}</slot>
      </div>
      <div class="list">
        <button
          v-for="it in items"
          :key="it.key"
          type="button"
          class="nav-item"
          :class="{ sel: it.key === modelValue }"
          :data-key="it.key"
          :aria-current="it.key === modelValue ? 'page' : undefined"
          :title="it.label"
          @click="emit('update:modelValue', it.key)"
        >
          <FIcon :name="it.icon" :size="16" />
          <span class="lbl">{{ it.label }}</span>
        </button>
      </div>
      <div v-if="footerItems?.length" class="list foot">
        <button
          v-for="it in footerItems"
          :key="it.key"
          type="button"
          class="nav-item"
          :class="{ sel: it.key === modelValue }"
          :data-key="it.key"
          :aria-current="it.key === modelValue ? 'page' : undefined"
          :title="it.label"
          @click="emit('update:modelValue', it.key)"
        >
          <FIcon :name="it.icon" :size="16" />
          <span class="lbl">{{ it.label }}</span>
        </button>
      </div>
      <span class="pill" :class="{ hidden: !pill.visible }" :style="{ transform: `translateY(${pill.top}px)` }" />
    </nav>
    <main class="content">
      <slot name="header">
        <h1 v-if="heading" class="page-title">{{ heading }}</h1>
      </slot>
      <slot />
    </main>
    </div>
  </div>
</template>

<style scoped>
.f-nav { container-type: inline-size; height: 100%; min-height: 0; }
.layout {
  display: grid;
  grid-template-columns: 280px minmax(0, 1fr);
  height: 100%;
  min-height: 0;
}
.pane {
  position: relative;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow-y: auto;
  padding: 4px 0 8px;
}
.pane-title { padding: 10px 16px 14px; font-size: 12px; line-height: 16px; color: var(--text-primary); }
.list { display: flex; flex-direction: column; }
.foot { margin-top: auto; padding-top: 8px; }
.nav-item {
  display: flex;
  align-items: center;
  gap: 16px;
  height: 36px;
  margin: 2px 8px 2px 4px;
  padding: 0 12px 0 14px;
  border: 0;
  border-radius: var(--radius-control);
  background: transparent;
  color: var(--text-primary);
  text-align: left;
  white-space: nowrap;
  overflow: hidden;
  transition: background-color var(--dur-fast) var(--ease);
}
.nav-item:hover { background: var(--subtle-fill-hover); }
.nav-item:active { background: var(--subtle-fill-pressed); color: var(--text-secondary); }
.nav-item.sel { background: var(--subtle-fill-hover); }
.nav-item.sel:hover { background: var(--subtle-fill-hover); }
.lbl { overflow: hidden; text-overflow: ellipsis; }
.pill {
  position: absolute;
  left: 4px;
  top: 0;
  width: 3px;
  height: 16px;
  border-radius: 2px;
  background: var(--accent-fill);
  transition: transform var(--dur-normal) var(--ease), opacity var(--dur-fast);
  pointer-events: none;
}
.pill.hidden { opacity: 0; }
.content {
  min-width: 0;
  min-height: 0;
  overflow-y: auto;
  padding: 0 40px 40px 28px;
}
.page-title {
  margin: 10px 0 24px;
  font-family: var(--font-display);
  font-size: 28px;
  line-height: 36px;
  font-weight: 600;
}

@container (max-width: 680px) {
  .layout { grid-template-columns: 48px minmax(0, 1fr); }
  .pane-title, .lbl { display: none; }
  .nav-item { width: 40px; margin-left: 4px; margin-right: 4px; padding: 0; justify-content: center; }
  .content { padding: 0 20px 24px 16px; }
}
</style>
