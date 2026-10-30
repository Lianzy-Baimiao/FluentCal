<script setup lang="ts">
import { computed } from 'vue';
import { encode } from 'uqr';

/** QR code as one SVG path (no v-html), dark on white so any phone camera reads it. */
const props = withDefaults(defineProps<{ text: string; size?: number; label?: string }>(), { size: 168 });

const code = computed(() => encode(props.text, { ecc: 'M', border: 2 }));
const path = computed(() => {
  const d: string[] = [];
  code.value.data.forEach((row, y) => row.forEach((on, x) => on && d.push(`M${x} ${y}h1v1h-1z`)));
  return d.join('');
});
</script>

<template>
  <svg class="qr" :width="size" :height="size" :viewBox="`0 0 ${code.size} ${code.size}`" shape-rendering="crispEdges" role="img" :aria-label="label ?? '二维码'">
    <rect :width="code.size" :height="code.size" fill="#fff" />
    <path :d="path" fill="#000" />
  </svg>
</template>

<style scoped>
.qr {
  display: block;
  border-radius: 6px;
}
</style>
