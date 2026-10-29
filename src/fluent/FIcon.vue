<script setup lang="ts">
import { computed } from 'vue';
import { GLYPHS, SVG_ICONS, type IconName } from './icons';

const props = withDefaults(defineProps<{ name: IconName | string; size?: number; title?: string }>(), {
  size: 16,
});

const svg = computed(() => (SVG_ICONS as Record<string, readonly string[]>)[props.name]);
const glyph = computed(() => (GLYPHS as Record<string, string>)[props.name] ?? '');
</script>

<template>
  <svg
    v-if="svg"
    class="f-icon f-icon-svg"
    :width="size"
    :height="size"
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    stroke-width="1"
    stroke-linejoin="round"
    :role="title ? 'img' : undefined"
    :aria-label="title"
    :aria-hidden="title ? undefined : 'true'"
  >
    <path v-for="(d, i) in svg" :key="i" :d="d" />
  </svg>
  <span
    v-else
    class="f-icon"
    :style="{ fontSize: `${size}px`, width: `${size}px`, height: `${size}px` }"
    :role="title ? 'img' : undefined"
    :aria-label="title"
    :aria-hidden="title ? undefined : 'true'"
  >{{ glyph }}</span>
</template>

<style scoped>
.f-icon-svg { display: inline-block; flex: none; overflow: visible; }
</style>
