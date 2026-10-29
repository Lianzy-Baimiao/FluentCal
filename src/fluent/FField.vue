<script setup lang="ts">
// A labelled form row: the label names the Fluent control(s) in the default slot.
import { provideField } from './field';

const props = withDefaults(
  defineProps<{ label: string; description?: string; top?: boolean; labelWidth?: number | string }>(),
  { labelWidth: 72 },
);
const ids = provideField(() => props.label, () => !!props.description);
</script>

<template>
  <div class="f-field" :class="{ top }">
    <span :id="ids.titleId" class="lbl" :style="{ width: typeof labelWidth === 'number' ? `${labelWidth}px` : labelWidth }">{{ label }}</span>
    <div class="ctl">
      <slot />
      <span v-if="description" :id="ids.descId" class="desc">{{ description }}</span>
    </div>
  </div>
</template>

<style scoped>
.f-field { display: flex; align-items: center; gap: 12px; min-height: 40px; }
.f-field.top { align-items: flex-start; }
.f-field.top .lbl { padding-top: 6px; }
.lbl { flex: none; color: var(--text-secondary); }
.ctl { flex: 1; min-width: 0; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
.desc { font-size: 12px; line-height: 16px; color: var(--text-secondary); }
</style>
