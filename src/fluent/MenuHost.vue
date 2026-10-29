<script setup lang="ts">
import MenuPanel from './MenuPanel.vue';
import { closeMenu, menuState } from './menu';
</script>

<template>
  <MenuPanel
    v-if="menuState.open"
    :key="menuState.id"
    :items="menuState.items"
    :anchor="menuState.at"
    :placement="menuState.placement"
    :depth="0"
    @close-all="closeMenu"
  />
</template>
