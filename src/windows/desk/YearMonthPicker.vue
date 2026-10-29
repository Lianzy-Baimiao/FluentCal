<script setup lang="ts">
import { ref, watch } from 'vue';
import { FButton, FFlyout } from '@/fluent';
import { parts, ymd } from '@/lib/date';
import { now } from '@/composables/now';
import { ui } from './state';

defineProps<{ anchor: HTMLElement | null }>();
const open = defineModel<boolean>('open', { default: false });
const year = ref(parts(ui.cursor)[0]);
watch(open, (o) => o && (year.value = parts(ui.cursor)[0]));

const months = Array.from({ length: 12 }, (_, i) => i + 1);
function pick(m: number) {
  ui.cursor = ymd(year.value, m, 1);
  open.value = false;
}
const isCurrent = (m: number) => ui.cursor.startsWith(`${year.value}-${String(m).padStart(2, '0')}`);
const isNow = (m: number) => now.value.startsWith(`${year.value}-${String(m).padStart(2, '0')}`);
</script>

<template>
  <FFlyout v-model:open="open" :anchor="anchor" placement="bottom-start" :width="248">
    <div class="ym" style="--text-glow: none">
      <div class="hd">
        <span class="y">{{ year }}年</span>
        <FButton variant="subtle" size="icon" icon="chevronUp" title="上一年" @click="year--" />
        <FButton variant="subtle" size="icon" icon="chevronDown" title="下一年" @click="year++" />
      </div>
      <div class="grid" role="listbox" :aria-label="`${year}年`">
        <button
          v-for="m in months"
          :key="m"
          type="button"
          role="option"
          :aria-selected="isCurrent(m)"
          :class="{ cur: isCurrent(m), now: isNow(m) }"
          @click="pick(m)"
        >
          {{ m }}月
        </button>
      </div>
    </div>
  </FFlyout>
</template>

<style scoped>
.hd {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-bottom: 6px;
}
.y {
  flex: 1;
  font-weight: 600;
  padding-left: 6px;
}
.grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 4px;
}
.grid button {
  height: 48px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-primary);
  font: inherit;
  cursor: pointer;
}
.grid button:hover {
  background: var(--subtle-fill-hover);
}
.grid button.now {
  color: var(--accent-text);
  font-weight: 600;
}
.grid button.cur {
  background: var(--accent-fill);
  color: var(--text-on-accent);
}
</style>
