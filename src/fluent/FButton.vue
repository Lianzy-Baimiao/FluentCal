<script setup lang="ts">
import FIcon from './FIcon.vue';

withDefaults(
  defineProps<{
    variant?: 'standard' | 'accent' | 'subtle';
    icon?: string;
    iconRight?: string;
    size?: 'medium' | 'small' | 'icon';
    disabled?: boolean;
    title?: string;
    /** Toggle-button state (adds aria-pressed and a selected look). */
    pressed?: boolean;
    type?: 'button' | 'submit';
  }>(),
  { variant: 'standard', size: 'medium', type: 'button', pressed: undefined },
);
</script>

<template>
  <button
    class="f-btn"
    :class="[`v-${variant}`, `s-${size}`, { on: pressed }]"
    :type="type"
    :disabled="disabled"
    :title="title"
    :aria-label="size === 'icon' ? title : undefined"
    :aria-pressed="pressed"
  >
    <FIcon v-if="icon" :name="icon" :size="size === 'small' ? 12 : 16" />
    <span v-if="$slots.default" class="label"><slot /></span>
    <FIcon v-if="iconRight" :name="iconRight" :size="12" />
  </button>
</template>

<style scoped>
.f-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 32px;
  padding: 5px 11px 6px;
  border: 1px solid transparent;
  border-radius: var(--radius-control);
  background: transparent;
  color: var(--text-primary);
  font-size: 14px;
  line-height: 20px;
  white-space: nowrap;
  flex: none;
  transition: background-color var(--dur-fast) var(--ease), border-color var(--dur-fast) var(--ease);
}
.label { min-width: 0; overflow: hidden; text-overflow: ellipsis; }

.v-standard {
  background: var(--control-fill);
  border-color: var(--elev-top);
  border-bottom-color: var(--elev-bottom);
}
.v-standard:hover:not(:disabled) { background: var(--control-fill-hover); }
.v-standard:active:not(:disabled) {
  background: var(--control-fill-pressed);
  border-color: var(--control-stroke);
  color: var(--text-secondary);
}

.v-accent {
  background: var(--accent-fill);
  color: var(--text-on-accent);
  border-color: var(--accent-elev-top);
  border-bottom-color: var(--accent-elev-bottom);
}
.v-accent:hover:not(:disabled) { background: var(--accent-fill-hover); }
.v-accent:active:not(:disabled) {
  background: var(--accent-fill-pressed);
  border-color: transparent;
  color: var(--text-on-accent-secondary);
}

.v-subtle:hover:not(:disabled), .v-subtle.on { background: var(--subtle-fill-hover); }
.v-subtle:active:not(:disabled) { background: var(--subtle-fill-pressed); color: var(--text-secondary); }
.v-subtle.on { color: var(--accent-text); }

.f-btn:disabled { color: var(--text-disabled); }
.v-standard:disabled { background: var(--control-fill-disabled); border-color: var(--control-stroke); }
.v-accent:disabled { background: var(--accent-fill-disabled); color: var(--text-on-accent-disabled); border-color: transparent; }

.s-small { min-height: 24px; padding: 1px 7px 2px; font-size: 12px; line-height: 16px; gap: 6px; }
.s-icon { width: 32px; height: 32px; min-height: 32px; padding: 0; }
.s-icon.v-subtle { width: 32px; height: 32px; }
</style>
