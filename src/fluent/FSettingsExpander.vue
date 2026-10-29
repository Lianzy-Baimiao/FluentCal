<script setup lang="ts">
import { ref, useSlots, watch } from 'vue';
import FIcon from './FIcon.vue';
import { provideField } from './field';

const props = withDefaults(
  defineProps<{ icon?: string; title: string; description?: string; disabled?: boolean; expanded?: boolean }>(),
  { expanded: false },
);
const emit = defineEmits<{ 'update:expanded': [boolean] }>();

const open = ref(props.expanded);
watch(() => props.expanded, (v) => (open.value = v));
const id = `fexp-${Math.random().toString(36).slice(2, 8)}`;
const slots = useSlots();
const ids = provideField(() => props.title, () => !!(props.description || slots.description));

function toggle() {
  if (props.disabled) return;
  open.value = !open.value;
  emit('update:expanded', open.value);
}

function headerClick(e: MouseEvent) {
  // Clicks on the header control (toggle, select…) must not expand/collapse.
  if ((e.target as HTMLElement).closest('.content, .chev-btn')) return;
  toggle();
}
</script>

<template>
  <div class="f-exp" :class="{ open, disabled, noicon: !icon }">
    <div class="hdr" @click="headerClick">
      <FIcon v-if="icon" :name="icon" :size="20" class="ico" />
      <div class="head">
        <div :id="ids.titleId" class="title">{{ title }}</div>
        <div v-if="description || $slots.description" :id="ids.descId" class="desc">
          <slot name="description">{{ description }}</slot>
        </div>
      </div>
      <div v-if="$slots.default" class="content"><slot /></div>
      <button
        type="button"
        class="chev-btn"
        :aria-expanded="open"
        :aria-controls="id"
        :aria-labelledby="ids.titleId"
        :aria-describedby="description || $slots.description ? ids.descId : undefined"
        :disabled="disabled"
        @click="toggle"
      >
        <FIcon name="chevronDown" :size="12" class="chev" />
      </button>
    </div>
    <div :id="id" class="body">
      <div class="inner" :inert="!open" :aria-hidden="!open">
        <div class="items"><slot name="items" /></div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.f-exp { --row-pad: 56px; }
.f-exp.noicon { --row-pad: 16px; }
.hdr {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  column-gap: 16px;
  row-gap: 8px;
  min-height: 68px;
  padding: 13px 8px 13px 16px;
  border: 1px solid var(--card-stroke);
  border-radius: var(--radius-control);
  background: var(--card-fill);
  transition: background-color var(--dur-fast) var(--ease), border-radius var(--dur-fast);
}
.hdr:hover { background: var(--control-fill-hover); }
.open .hdr { border-radius: var(--radius-control) var(--radius-control) 0 0; }
.ico { margin-right: 4px; }
.head { flex: 1 1 200px; min-width: 0; }
.title { font-size: 14px; line-height: 20px; }
.desc { font-size: 12px; line-height: 16px; color: var(--text-secondary); margin-top: 1px; }
.content { display: flex; align-items: center; gap: 8px; margin-left: auto; flex: none; max-width: 100%; }
.chev-btn {
  width: 32px;
  height: 32px;
  margin-left: -8px;
  padding: 0;
  border: 0;
  border-radius: var(--radius-control);
  background: transparent;
  color: var(--text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
}
.chev-btn:hover:not(:disabled) { background: var(--subtle-fill-hover); }
.chev-btn:active:not(:disabled) { background: var(--subtle-fill-pressed); }
.chev { transition: transform var(--dur-normal) var(--ease); }
.open .chev { transform: rotate(180deg); }
.body { display: grid; grid-template-rows: 0fr; transition: grid-template-rows var(--dur-normal) var(--ease); }
.open .body { grid-template-rows: 1fr; }
.inner { overflow: hidden; min-height: 0; }
.items {
  border: 1px solid var(--card-stroke);
  border-top: 0;
  border-radius: 0 0 var(--radius-control) var(--radius-control);
  background: var(--card-fill-secondary);
}
.disabled .title, .disabled .desc, .disabled .chev-btn { color: var(--text-disabled); }
</style>
