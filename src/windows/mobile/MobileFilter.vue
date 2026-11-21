<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { taskColor } from "@/fluent";
import { filterRows, toggleFilter } from "@/lib/filter";
import { doc, taskList } from "@/stores/doc";
import { isDark } from "@/composables/theme";

/**
 * 按分组筛选 (CAL-20): the groups to show on the 日程 and 月历 pages, any number of them, 未分组
 * among them; 全部 shows everything again. The choice is the caller's to keep (for the session
 * only: it is for a look).
 */
const props = defineProps<{ open: boolean; filter: string[] }>();
const emit = defineEmits<{ close: []; "update:filter": [value: string[]] }>();

const rows = computed(() => filterRows(taskList.value, doc.groups, props.filter));
const panel = ref<HTMLElement>();
let restoreFocus: HTMLElement | null = null;
watch(
  () => props.open,
  async (v) => {
    if (v) {
      restoreFocus = document.activeElement as HTMLElement;
      await nextTick();
      panel.value?.focus();
    } else restoreFocus?.focus();
  },
);
const toggle = (id: string, on: boolean) => emit("update:filter", toggleFilter(doc.groups, props.filter, id, on));
</script>

<template>
  <Transition name="sheet">
    <div v-if="open" class="sheet-layer" @click.self="emit('close')">
      <section
        ref="panel"
        class="sheet"
        tabindex="-1"
        role="dialog"
        aria-modal="true"
        aria-label="按分组筛选"
        @keydown.esc.prevent="emit('close')"
      >
        <header class="sheet-head">
          <h2>按分组筛选</h2>
          <button type="button" class="done-btn" @click="emit('close')">完成</button>
        </header>
        <div class="rows">
          <button
            type="button"
            class="row"
            role="checkbox"
            :aria-checked="!filter.length"
            @click="emit('update:filter', [])"
          >
            <span class="box" :class="{ on: !filter.length }" aria-hidden="true">✓</span
            ><span class="name">全部</span><small>{{ taskList.length }}</small>
          </button>
          <div class="sep" role="separator" />
          <button
            v-for="r in rows"
            :key="r.id"
            type="button"
            class="row"
            role="checkbox"
            :aria-checked="r.checked"
            @click="toggle(r.id, !r.checked)"
          >
            <span class="box" :class="{ on: r.checked }" aria-hidden="true">✓</span
            ><i v-if="r.color" class="dot" :style="{ background: taskColor(r.color, isDark).bar }" aria-hidden="true" /><span
              class="name"
              >{{ r.name }}</span
            ><small>{{ r.count }}</small>
          </button>
          <p v-if="rows.length === 1" class="hint">还没有分组。编辑任务时可以新建分组。</p>
          <p class="hint">日程和月历只显示勾选分组的任务；甘特图和搜索不受影响。不会保存，重新打开应用后显示全部。</p>
        </div>
      </section>
    </div>
  </Transition>
</template>

<style scoped>
.sheet-layer {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: flex-end;
  background: rgba(0, 0, 0, 0.42);
}
.sheet {
  width: 100%;
  max-height: min(80dvh, 640px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-radius: 24px 24px 0 0;
  background: var(--solid-base);
  color: var(--text-primary);
  box-shadow: 0 -12px 36px rgba(0, 0, 0, 0.22);
  outline: 0;
}
.sheet-head {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 18px 12px;
  border-bottom: 1px solid var(--divider);
}
.sheet-head h2 {
  flex: 1;
  margin: 0;
  font-size: 17px;
}
.done-btn {
  min-height: 36px;
  padding: 7px 2px;
  border: 0;
  background: transparent;
  color: var(--accent-text);
  font-weight: 650;
}
.rows {
  overflow-y: auto;
  padding: 8px 12px calc(18px + env(safe-area-inset-bottom));
}
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  min-height: 48px;
  padding: 6px 8px;
  border: 0;
  border-radius: 10px;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
}
.row:active {
  background: var(--subtle-fill-pressed);
}
.box {
  display: grid;
  place-items: center;
  flex: none;
  width: 22px;
  height: 22px;
  border: 1.5px solid var(--control-strong-fill);
  border-radius: 6px;
  color: transparent;
  font-size: 13px;
}
.box.on {
  border-color: var(--accent-fill);
  background: var(--accent-fill);
  color: var(--text-on-accent, #fff);
}
.dot {
  flex: none;
  width: 10px;
  height: 10px;
  border-radius: 3px;
}
.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 15px;
}
.row small {
  flex: none;
  color: var(--text-secondary);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
.sep {
  height: 1px;
  margin: 4px 8px;
  background: var(--divider);
}
.hint {
  margin: 8px 8px 0;
  color: var(--text-secondary);
  font-size: 12px;
  line-height: 1.6;
}
.sheet-enter-active,
.sheet-leave-active {
  transition: opacity 0.22s ease;
}
.sheet-enter-active .sheet,
.sheet-leave-active .sheet {
  transition: transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1);
}
.sheet-enter-from,
.sheet-leave-to {
  opacity: 0;
}
.sheet-enter-from .sheet,
.sheet-leave-to .sheet {
  transform: translateY(100%);
}
@media (min-width: 700px) {
  .sheet-layer {
    align-items: center;
    justify-content: center;
  }
  .sheet {
    width: min(440px, 94vw);
    border-radius: 22px;
  }
}
</style>
