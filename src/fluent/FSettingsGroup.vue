<script setup lang="ts">
defineProps<{ title?: string }>();
</script>

<template>
  <section class="f-group">
    <h2 v-if="title" class="title">{{ title }}</h2>
    <div class="stack"><slot /></div>
  </section>
</template>

<style scoped>
.f-group + .f-group { margin-top: 28px; }
.title { margin: 0 0 8px 1px; font-size: 14px; line-height: 20px; font-weight: 600; }
.stack { display: flex; flex-direction: column; gap: 4px; }
</style>
