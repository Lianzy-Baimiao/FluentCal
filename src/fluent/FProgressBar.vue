<script setup lang="ts">
import { useFieldName } from './field';

const props = defineProps<{ value?: number; label?: string }>();
const name = useFieldName(() => props.label, () => '进度');
</script>

<template>
  <div
    class="f-progress"
    :class="{ indet: value === undefined }"
    role="progressbar"
    :aria-valuenow="value"
    aria-valuemin="0"
    aria-valuemax="100"
    v-bind="name.attrs()"
  >
    <div class="bar" :style="value === undefined ? undefined : { width: `${Math.min(100, Math.max(0, value))}%` }" />
  </div>
</template>

<style scoped>
.f-progress {
  position: relative;
  height: 3px;
  overflow: hidden;
  display: flex;
  align-items: center;
}
.f-progress::before {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  top: 50%;
  height: 1px;
  background: var(--control-strong-stroke);
}
.bar {
  position: relative;
  height: 3px;
  border-radius: 2px;
  background: var(--accent-fill);
  transition: width var(--dur-normal) var(--ease);
}
.indet::before { opacity: 0; }
.indet .bar { width: 40%; animation: f-indet 2s infinite cubic-bezier(0.4, 0, 0.6, 1); }
@keyframes f-indet {
  from { transform: translateX(-100%); }
  to { transform: translateX(250%); }
}
</style>
