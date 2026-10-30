<script setup lang="ts">
import { computed } from 'vue';
import { GLYPHS, SVG_ICONS, WEB_SVG, type IconName } from './icons';

const props = withDefaults(defineProps<{ name: IconName | string; size?: number; title?: string }>(), {
  size: 16,
});

/** The web view (phones) has no icon font. */
const web = typeof window !== 'undefined' && !('__TAURI_INTERNALS__' in window) && new URLSearchParams(location.search).get('w') === 'web';
const svg = computed(() => (SVG_ICONS as Record<string, readonly string[]>)[props.name] ?? (web ? (WEB_SVG as Record<string, readonly string[]>)[props.name] : undefined));
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
