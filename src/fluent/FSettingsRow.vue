<script setup lang="ts">
import { useSlots } from 'vue';
import { provideField } from './field';

const props = defineProps<{ title: string; description?: string }>();
const slots = useSlots();
const ids = provideField(() => props.title, () => !!(props.description || slots.description));
</script>

<template>
  <div class="f-row">
    <div class="head">
      <div :id="ids.titleId" class="title">{{ title }}</div>
      <div v-if="description || $slots.description" :id="ids.descId" class="desc">
        <slot name="description">{{ description }}</slot>
      </div>
    </div>
    <div v-if="$slots.default" class="content"><slot /></div>
  </div>
</template>

<style scoped>
.f-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  column-gap: 16px;
  row-gap: 8px;
  min-height: 52px;
  padding: 8px 48px 8px var(--row-pad, 56px);
}
.f-row + .f-row { border-top: 1px solid var(--card-stroke); }
.head { flex: 1 1 200px; min-width: 0; }
.title { font-size: 14px; line-height: 20px; }
.desc { font-size: 12px; line-height: 16px; color: var(--text-secondary); margin-top: 1px; }
.content { display: flex; align-items: center; gap: 8px; margin-left: auto; flex: none; max-width: 100%; }
</style>
