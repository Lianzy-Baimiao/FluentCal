<script setup lang="ts">
import { useSlots } from 'vue';
import FIcon from './FIcon.vue';
import { provideField } from './field';

const props = defineProps<{ icon?: string; title: string; description?: string; clickable?: boolean; disabled?: boolean }>();
const emit = defineEmits<{ click: [MouseEvent] }>();
const slots = useSlots();
const ids = provideField(() => props.title, () => !!(props.description || slots.description));
</script>

<template>
  <component
    :is="clickable ? 'button' : 'div'"
    class="f-card"
    :class="{ clickable, disabled }"
    :type="clickable ? 'button' : undefined"
    :disabled="clickable && disabled ? true : undefined"
    @click="clickable && !disabled && emit('click', $event)"
  >
    <FIcon v-if="icon" :name="icon" :size="20" class="ico" />
    <div class="head">
      <div :id="ids.titleId" class="title">{{ title }}</div>
      <div v-if="description || $slots.description" :id="ids.descId" class="desc">
        <slot name="description">{{ description }}</slot>
      </div>
    </div>
    <div v-if="$slots.default" class="content"><slot /></div>
    <FIcon v-if="clickable" name="chevronRight" :size="12" class="chev" />
  </component>
</template>

<style scoped>
.f-card {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  column-gap: 16px;
  row-gap: 8px;
  width: 100%;
  min-height: 68px;
  padding: 13px 16px;
  border: 1px solid var(--card-stroke);
  border-radius: var(--radius-control);
  background: var(--card-fill);
  color: var(--text-primary);
  text-align: left;
  font: inherit;
}
.ico { margin-right: 4px; }
.head { flex: 1 1 200px; min-width: 0; }
.title { font-size: 14px; line-height: 20px; }
.desc { font-size: 12px; line-height: 16px; color: var(--text-secondary); margin-top: 1px; }
.content { display: flex; align-items: center; gap: 8px; margin-left: auto; flex: none; max-width: 100%; }
.chev { color: var(--text-secondary); margin-left: 4px; }
.clickable { transition: background-color var(--dur-fast) var(--ease); }
.clickable:hover:not(:disabled) { background: var(--control-fill-hover); }
.clickable:active:not(:disabled) { background: var(--control-fill-pressed); }
.disabled, .disabled .desc, .disabled .chev { color: var(--text-disabled); }
</style>
