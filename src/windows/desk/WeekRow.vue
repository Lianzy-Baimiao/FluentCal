<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { vTip } from '@/fluent';
import type { ISODate } from '@/types';
import type { Chip, WeekLayout } from '@/lib/layout';
import { addTask } from '@/lib/tasks';
import { storeState } from '@/stores/doc';
import { notify, ui } from './state';
import type { CellInfo } from './types';
import TaskChip from './TaskChip.vue';

const props = defineProps<{
  cells: CellInfo[];
  layout: WeekLayout;
  chipH: number;
  laneH: number;
  headH: number;
  weekNo?: number;
  dropDate?: ISODate;
  draggingKey?: string;
}>();

const emit = defineEmits<{
  chipToggle: [c: Chip];
  chipOpen: [c: Chip];
  chipMenu: [c: Chip, e: MouseEvent];
  chipDown: [c: Chip, e: PointerEvent];
  chipKey: [c: Chip, e: KeyboardEvent];
  cellMenu: [date: ISODate, e: MouseEvent];
  more: [date: ISODate, el: HTMLElement];
}>();

// Chips live inside the cell of their first day and stretch over the following cells, so
// the grid stays a valid ARIA grid (rows contain only cells).
const byCol = computed(() => props.cells.map((_, i) => props.layout.chips.filter((c) => c.visible && c.col === i)));

function chipStyle(c: Chip) {
  const l = c.contLeft ? 0 : 2;
  const r = c.contRight ? 0 : 2;
  return {
    left: `${l}px`,
    width: `calc(${c.span} * 100% + ${c.span - 1} * var(--cell-gap) - ${l + r}px)`,
    top: `${props.headH + c.lane * props.laneH}px`,
    height: `${props.chipH}px`,
  };
}

function moreStyle(col: number) {
  return { top: `${props.headH + props.layout.overflowLane[col] * props.laneH}px`, height: `${props.chipH}px` };
}

// Quick add: an inline box at the bottom of the day cell; stays open for the next entry.
const qaText = ref('');
const root = ref<HTMLElement>();
const qaCol = () => props.cells.findIndex((c) => c.date === ui.quickAdd?.date);
watch(
  () => ui.quickAdd?.date,
  async () => {
    if (qaCol() < 0) return;
    qaText.value = '';
    await nextTick();
    root.value?.querySelector<HTMLInputElement>('input.quick-add')?.focus();
  },
  { immediate: true },
);

async function qaSubmit() {
  const title = qaText.value.trim();
  const date = ui.quickAdd?.date;
  if (!title || !date) return;
  qaText.value = '';
  await addTask({ title, start: date });
  await nextTick();
  const col = qaCol();
  if (col >= 0 && props.layout.overflow[col] > 0) notify(`已添加“${title}”（点 +${props.layout.overflow[col]} 查看）`);
}

function qaClose() {
  ui.quickAdd = null;
}

function onCellDblClick(date: ISODate) {
  if (!storeState.readOnly) ui.quickAdd = { date };
}
</script>

<template>
  <div ref="root" class="row" role="row">
    <div v-if="weekNo !== undefined" class="weekno" aria-hidden="true">{{ weekNo }}</div>
    <div class="area">
      <div class="cells">
        <div
          v-for="(c, i) in cells"
          :key="c.date"
          class="cell"
          role="gridcell"
          :data-date="c.date"
          :tabindex="ui.selected === c.date ? 0 : -1"
          :aria-selected="ui.selected === c.date"
          :aria-describedby="`desc-${c.date}`"
          :class="{ rest: c.rest, today: c.today, dim: c.dim, marked: !!c.mark, drop: dropDate === c.date, flash: ui.flash === c.date }"
          :style="c.mark ? { '--mark': c.mark } : undefined"
          @click="ui.selected = c.date"
          @dblclick="onCellDblClick(c.date)"
          @contextmenu.prevent="emit('cellMenu', c.date, $event)"
        >
          <div class="head" :style="{ height: `${headH}px` }" v-tip="{ text: c.tip, placement: 'top' }">
            <span class="num">{{ c.label }}</span>
            <span v-if="c.lunar" class="lunar" :class="c.lunarKind">{{ c.lunar }}</span>
          </div>
          <span v-if="c.badge" class="badge" :class="c.badge === '休' ? 'off' : 'work'">{{ c.badge }}</span>
          <span :id="`desc-${c.date}`" class="sr-only">{{ c.aria }}</span>
          <div v-for="ch in byCol[i]" :key="ch.key" class="slot" :style="chipStyle(ch)">
            <TaskChip
              :chip="ch"
              :dragging="draggingKey === ch.key"
              @toggle="emit('chipToggle', ch)"
              @open="emit('chipOpen', ch)"
              @menu="emit('chipMenu', ch, $event)"
              @pointerdown="emit('chipDown', ch, $event)"
              @key="emit('chipKey', ch, $event)"
            />
          </div>
          <button
            v-if="layout.overflow[i] > 0"
            class="more"
            type="button"
            :style="moreStyle(i)"
            :aria-label="`还有 ${layout.overflow[i]} 项`"
            @dblclick.stop
            @click.stop="emit('more', c.date, $event.currentTarget as HTMLElement)"
          >
            +{{ layout.overflow[i] }}
          </button>
          <input
            v-if="ui.quickAdd?.date === c.date"
            v-model="qaText"
            class="quick-add"
            :style="{ height: `${chipH + 6}px` }"
            placeholder="添加任务，回车保存"
            maxlength="200"
            aria-label="快速添加任务"
            @click.stop
            @dblclick.stop
            @keydown.enter.prevent="qaSubmit"
            @keydown.esc.prevent="qaClose"
            @blur="qaClose"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.row {
  display: flex;
  min-height: 0;
  gap: var(--cell-gap);
}
.weekno {
  width: 18px;
  flex: none;
  font-size: calc(10px * var(--fs));
  color: var(--text-tertiary);
  text-align: center;
  padding-top: 4px;
  font-variant-numeric: tabular-nums;
  text-shadow: var(--text-glow);
}
.area {
  position: relative;
  flex: 1;
  min-width: 0;
}
.cells {
  position: absolute;
  inset: 0;
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  gap: var(--cell-gap);
}
.cell {
  position: relative;
  border-radius: 4px;
  background: var(--cell-bg);
  min-width: 0;
  outline: none;
  transition: background-color var(--dur-fast) var(--ease);
}
.cell.rest {
  background-image: linear-gradient(var(--rest-tint), var(--rest-tint));
}
.cell.marked {
  background-image: linear-gradient(color-mix(in srgb, var(--mark) 30%, transparent), color-mix(in srgb, var(--mark) 30%, transparent));
}
.cell.today {
  background-image: linear-gradient(color-mix(in srgb, var(--today) 12%, transparent), color-mix(in srgb, var(--today) 12%, transparent));
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--today) 55%, transparent);
}
.cell:focus-visible {
  box-shadow:
    inset 0 0 0 2px var(--focus-outer),
    inset 0 0 0 3px var(--focus-inner);
}
.cell.drop {
  box-shadow: inset 0 0 0 2px var(--accent-fill);
}
.cell.flash {
  animation: flash 1.5s var(--ease);
}
@keyframes flash {
  0%,
  60% {
    box-shadow: inset 0 0 0 2px var(--accent-fill);
  }
  100% {
    box-shadow: inset 0 0 0 0 transparent;
  }
}
.head {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0 4px;
  white-space: nowrap;
  overflow: hidden;
  text-shadow: var(--text-glow);
}
.num {
  font-size: calc(13px * var(--fs));
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  min-width: calc(18px * var(--fs));
  height: calc(18px * var(--fs));
  line-height: calc(18px * var(--fs));
  text-align: center;
  border-radius: 9px;
  flex: none;
}
.rest .num {
  color: var(--holiday-text);
}
.today .num {
  background: var(--today);
  color: var(--text-on-accent);
  text-shadow: none;
  padding: 0 4px;
}
.lunar {
  font-size: calc(11px * var(--fs));
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
}
.lunar.festival,
.lunar.term {
  color: var(--term-text);
}
.lunar.month {
  color: var(--text-primary);
}
/* Days of the adjacent months: lighter by colour and weight, not opacity, to keep 4.5:1. */
.dim .num {
  font-weight: 400;
  color: var(--text-secondary);
}
.dim.rest .num {
  color: var(--holiday-text);
}
.dim .lunar,
.dim .lunar.month {
  color: var(--text-secondary);
}
.dim {
  background-color: color-mix(in srgb, var(--cell-bg) 55%, transparent);
}
.badge {
  position: absolute;
  top: 2px;
  right: 2px;
  font-size: calc(10px * var(--fs));
  line-height: calc(14px * var(--fs));
  padding: 0 2px;
  border-radius: 3px;
  font-weight: 600;
}
.badge.off {
  color: var(--holiday-text);
  background: color-mix(in srgb, var(--holiday-text) 14%, transparent);
}
.badge.work {
  color: var(--text-secondary);
  background: var(--subtle-fill-hover);
}
.slot,
.more,
.quick-add {
  position: absolute;
  z-index: 1;
}
.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
}
.more {
  left: 2px;
  right: 2px;
  border: 0;
  margin: 0;
  padding: 0 4px;
  text-align: left;
  font: inherit;
  font-size: calc(11px * var(--fs));
  color: var(--text-secondary);
  background: transparent;
  border-radius: 4px;
  cursor: pointer;
  text-shadow: var(--text-glow);
}
.more:hover {
  background: var(--subtle-fill-hover);
  color: var(--text-primary);
}
.quick-add {
  left: 1px;
  width: calc(100% - 2px);
  bottom: 3px;
  z-index: 2;
  box-sizing: border-box;
  font: inherit;
  font-size: calc(12px * var(--fs));
  padding: 0 6px;
  border-radius: 4px;
  border: 1px solid var(--control-stroke);
  border-bottom: 2px solid var(--accent-fill);
  background: var(--solid-base);
  color: var(--text-primary);
  outline: none;
  box-shadow: var(--shadow-flyout);
  user-select: text;
}
.quick-add::placeholder {
  color: var(--text-tertiary);
}
</style>
