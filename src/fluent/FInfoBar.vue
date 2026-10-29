<script setup lang="ts">
import { computed } from 'vue';
import FIcon from './FIcon.vue';

const props = withDefaults(
  defineProps<{ severity?: 'info' | 'success' | 'warning' | 'error'; title?: string; closable?: boolean }>(),
  { severity: 'info' },
);
const emit = defineEmits<{ close: [] }>();

const glyph = computed(() => ({
  info: 'statusInfo',
  success: 'statusCheck',
  warning: 'statusExclamation',
  error: 'statusX',
}[props.severity]));
</script>

<template>
  <div class="f-infobar" :class="severity" :role="severity === 'error' || severity === 'warning' ? 'alert' : 'status'">
    <span class="ico" aria-hidden="true">
      <FIcon name="statusCircle" :size="16" class="bg" />
      <FIcon :name="glyph" :size="16" class="fg" />
    </span>
    <div class="body">
      <span v-if="title" class="title">{{ title }}</span>
      <span class="msg"><slot /></span>
      <span v-if="$slots.action" class="action"><slot name="action" /></span>
    </div>
    <button v-if="closable" type="button" class="close" aria-label="关闭" @click="emit('close')">
      <FIcon name="close" :size="12" />
    </button>
  </div>
</template>

<style scoped>
.f-infobar {
  display: flex;
  align-items: center;
  gap: 16px;
  min-height: 48px;
  padding: 0 0 0 15px;
  border: 1px solid var(--card-stroke);
  border-radius: var(--radius-control);
  background: var(--info-bg);
  color: var(--text-primary);
}
.success { background: var(--success-bg); }
.warning { background: var(--caution-bg); }
.error { background: var(--critical-bg); }
.ico { position: relative; width: 16px; height: 16px; flex: none; }
.ico :deep(.f-icon) { position: absolute; inset: 0; }
.bg { color: var(--accent-fill); }
.success .bg { color: var(--success); }
.warning .bg { color: var(--caution); }
.error .bg { color: var(--critical); }
.fg { color: var(--text-inverse); }
.body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  column-gap: 12px;
  row-gap: 4px;
  padding: 13px 0 13px;
}
.title { font-weight: 600; }
.msg { min-width: 0; }
.action { margin-left: auto; }
.close {
  width: 38px;
  height: 38px;
  margin: 0 4px 0 0;
  align-self: flex-start;
  margin-top: 4px;
  border: 0;
  padding: 0;
  border-radius: var(--radius-control);
  background: transparent;
  color: var(--text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
}
.close:hover { background: var(--subtle-fill-hover); }
.close:active { background: var(--subtle-fill-pressed); color: var(--text-secondary); }
</style>
