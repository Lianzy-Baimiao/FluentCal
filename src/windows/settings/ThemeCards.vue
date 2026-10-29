<script setup lang="ts">
import type { ThemeMode } from '@/types';
import { doc, patchSettings } from '@/stores/doc';
import { matchPreset, PRESETS } from './presets';

const modes: { v: ThemeMode; label: string }[] = [
  { v: 'light', label: '浅色' },
  { v: 'dark', label: '深色' },
  { v: 'system', label: '跟随系统' },
];
defineProps<{ kind: 'mode' | 'preset' }>();
</script>

<template>
  <div v-if="kind === 'mode'" class="cards" role="radiogroup" aria-label="颜色模式">
    <button
      v-for="m in modes"
      :key="m.v"
      type="button"
      role="radio"
      class="card"
      :aria-checked="doc.settings.theme.mode === m.v"
      :class="{ on: doc.settings.theme.mode === m.v }"
      @click="patchSettings({ theme: { mode: m.v } })"
    >
      <span class="thumb" :class="m.v">
        <span class="win"><i /><b /><b /><b class="short" /></span>
      </span>
      <span class="label">{{ m.label }}</span>
    </button>
  </div>
  <div v-else class="cards" role="radiogroup" aria-label="预设方案">
    <button
      v-for="p in PRESETS"
      :key="p.id"
      type="button"
      role="radio"
      class="card"
      :aria-checked="matchPreset(doc.settings) === p.id"
      :class="{ on: matchPreset(doc.settings) === p.id }"
      @click="patchSettings(p.patch)"
    >
      <span class="thumb wall">
        <span class="mini" :class="{ dark: p.thumb.dark, acr: p.thumb.acrylic }" :style="{ '--bg': p.thumb.bg, '--cell': p.thumb.cell }">
          <span v-for="i in 21" :key="i" :class="{ today: i === 10 }" />
        </span>
      </span>
      <span class="label">{{ p.label }}</span>
      <span class="desc">{{ p.desc }}</span>
    </button>
  </div>
</template>

<style scoped>
.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
  gap: 12px;
  padding: 4px 0 12px;
}
.card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 6px 6px 8px;
  border: 1px solid var(--card-stroke);
  border-radius: 8px;
  background: var(--card-fill);
  color: var(--text-primary);
  font: inherit;
  cursor: pointer;
}
.card:hover {
  background: var(--control-fill-hover);
}
.card.on {
  border-color: var(--accent-fill);
  box-shadow: inset 0 0 0 1px var(--accent-fill);
}
.thumb {
  width: 100%;
  aspect-ratio: 132 / 80;
  border-radius: 4px;
  overflow: hidden;
  position: relative;
  display: grid;
  place-items: center;
  background: linear-gradient(160deg, #cfe2fb, #5d93e0);
}
.thumb.dark {
  background: linear-gradient(160deg, #0b2a5b, #1b4f9c);
}
.thumb.system {
  background: linear-gradient(115deg, #cfe2fb 50%, #0b2a5b 50%);
}
.win {
  width: 92px;
  height: 56px;
  border-radius: 4px;
  padding: 6px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 5px;
  background: #f3f3f3;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.25);
}
.dark .win {
  background: #202020;
}
.system .win {
  background: linear-gradient(115deg, #f3f3f3 50%, #202020 50%);
}
.win i {
  height: 6px;
  width: 40%;
  border-radius: 2px;
  background: var(--accent-fill);
}
.win b {
  height: 5px;
  border-radius: 2px;
  background: rgba(128, 128, 128, 0.45);
}
.win b.short {
  width: 60%;
}
.thumb.wall {
  background: linear-gradient(160deg, #0b2a5b, #1b4f9c 55%, #3a7bd5);
}
.mini {
  width: 84%;
  padding: 5px;
  border-radius: 4px;
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 2px;
  background: rgba(243, 243, 243, var(--bg));
}
.mini.dark {
  background: rgba(32, 32, 32, var(--bg));
}
.mini.acr {
  backdrop-filter: blur(6px);
}
.mini span {
  height: 14px;
  border-radius: 2px;
  background: rgba(255, 255, 255, calc(var(--cell) * 0.9));
}
.mini.dark span {
  background: rgba(60, 60, 60, var(--cell));
}
.mini span.today {
  box-shadow: inset 0 0 0 1.5px var(--accent-fill);
}
.label {
  font-size: 13px;
}
.desc {
  font-size: 11px;
  color: var(--text-secondary);
  margin-top: -4px;
}
</style>
