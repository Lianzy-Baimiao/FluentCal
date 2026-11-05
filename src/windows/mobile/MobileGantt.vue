<script setup lang="ts">
import { computed, ref } from "vue";
import { taskColor } from "@/fluent/theme";
import { addDays, diffDays, parts } from "@/lib/date";
import { buildRows } from "@/lib/gantt";
import { doc, groupList, taskList } from "@/stores/doc";
import { now } from "@/composables/now";
import { isDark } from "@/composables/theme";
import type { Task } from "@/types";

const emit = defineEmits<{ edit: [task: Task]; create: [groupId?: string] }>();
const props = defineProps<{ search: string }>();
const start = ref(addDays(now.value, -3));
const scale = ref("day");
const group = ref("");
const days = computed(() => (scale.value === "day" ? 28 : 84));
const width = computed(() => (scale.value === "day" ? 32 : 14));
const end = computed(() => addDays(start.value, days.value - 1));
const rows = computed(() =>
  buildRows(
    taskList.value.filter((t) =>
      `${t.title} ${t.notes ?? ""}`
        .toLowerCase()
        .includes(props.search.toLowerCase()),
    ),
    groupList.value.map((g) => ({ ...g, collapsed: false })),
    {
      hideDone: false,
      hideSingleDay: false,
      groupFilter: group.value ? [group.value] : [],
    },
  ),
);
const ticks = computed(() =>
  Array.from(
    { length: Math.ceil(days.value / (scale.value === "day" ? 1 : 7)) },
    (_, i) => addDays(start.value, i * (scale.value === "day" ? 1 : 7)),
  ),
);
const bar = (a: string, b: string) => ({
  left: `${Math.max(0, diffDays(start.value, a)) * width.value}px`,
  width: `${Math.max(0, Math.min(days.value, diffDays(start.value, b) + 1) - Math.max(0, diffDays(start.value, a))) * width.value}px`,
});
const visible = (t: Task) => t.end >= start.value && t.start <= end.value;
const todayLeft = computed(
  () => diffDays(start.value, now.value) * width.value,
);
const links = computed(() => {
  const out: string[] = [];
  rows.value.forEach((r, i) => {
    if (r.kind !== "task" || !visible(r.task)) return;
    for (const id of r.task.deps ?? []) {
      const j = rows.value.findIndex(
        (row) => row.kind === "task" && row.task.id === id,
      );
      const t = doc.tasks[id];
      if (j < 0 || !t || !visible(t)) continue;
      const x1 =
        Math.min(days.value, diffDays(start.value, t.end) + 1) * width.value;
      const x2 = Math.max(0, diffDays(start.value, r.task.start)) * width.value;
      const y1 = j * 52 + 26,
        y2 = i * 52 + 26;
      out.push(
        `M ${x1} ${y1} H ${x1 + 6} V ${y2 - 20} H ${x2 - 6} V ${y2} H ${x2}`,
      );
    }
  });
  return out;
});
</script>

<template>
  <section class="mg">
    <header class="tools">
      <h2>项目甘特图</h2>
      <button @click="emit('create')">＋ 任务</button>
    </header>
    <div class="controls">
      <button
        aria-label="上一个时间段"
        @click="start = addDays(start, -days / 2)"
      >
        ‹
      </button>
      <strong>{{ start.slice(0, 7) }}</strong>
      <button
        aria-label="下一个时间段"
        @click="start = addDays(start, days / 2)"
      >
        ›
      </button>
      <button @click="start = addDays(now, -3)">今天</button>
      <select v-model="scale" aria-label="时间刻度">
        <option value="day">按日</option>
        <option value="week">按周</option>
      </select>
    </div>
    <select v-model="group" class="group-filter" aria-label="筛选分组">
      <option value="">全部分组</option>
      <option v-for="g in groupList" :key="g.id" :value="g.id">
        {{ g.name }}
      </option>
      <option value="__ungrouped__">未分组</option>
    </select>
    <p class="hint">
      左右滑动时间轴，点任务编辑日期和进度。重复任务显示系列的起始安排。
    </p>
    <p v-if="!rows.length" class="empty">
      还没有项目，点击「＋ 任务」开始安排。
    </p>
    <div
      v-else
      class="scroll"
      role="region"
      aria-label="甘特图时间轴"
      tabindex="0"
    >
      <div
        class="canvas"
        :style="{ width: `${128 + days * width}px`, '--day': `${width}px` }"
      >
        <div class="head">
          <span class="name">任务 / 进度</span>
          <div class="ticks">
            <span
              v-for="d in ticks"
              :key="d"
              :style="{ width: `${width * (scale === 'day' ? 1 : 7)}px` }"
              >{{ parts(d)[1] }}/{{ parts(d)[2] }}</span
            >
          </div>
        </div>
        <div class="rows">
          <svg
            class="dependencies"
            :width="days * width"
            :height="rows.length * 52"
            aria-hidden="true"
          >
            <defs>
              <marker
                id="mobile-arrow"
                markerWidth="6"
                markerHeight="6"
                refX="5"
                refY="3"
                orient="auto"
              >
                <path d="M0 0L6 3L0 6Z" fill="currentColor" />
              </marker>
            </defs>
            <path
              v-for="(d, i) in links"
              :key="i"
              :d="d"
              fill="none"
              stroke="currentColor"
              marker-end="url(#mobile-arrow)"
            />
          </svg>
          <div
            v-if="todayLeft >= 0 && todayLeft < days * width"
            class="today-line"
            :style="{ left: `${128 + todayLeft}px` }"
          />
          <div v-for="r in rows" :key="r.key" class="row" :class="r.kind">
            <template v-if="r.kind === 'group'"
              ><strong class="name"
                >{{ r.group.name }}<small>{{ r.progress }}%</small></strong
              >
              <div class="track">
                <span
                  v-if="r.start && r.end"
                  class="summary"
                  :style="bar(r.start, r.end)"
                /></div
            ></template>
            <template v-else-if="r.kind === 'task'">
              <button class="name task-name" @click="emit('edit', r.task)">
                {{ r.task.title
                }}<small
                  >{{ r.task.done ? 100 : r.task.progress }}%{{
                    r.task.repeat ? " · 重复" : ""
                  }}</small
                >
              </button>
              <div class="track">
                <button
                  v-if="visible(r.task)"
                  class="bar"
                  :class="{ milestone: r.task.milestone }"
                  :style="{
                    ...bar(r.task.start, r.task.end),
                    '--bar': taskColor(r.task.color, isDark).bar,
                  }"
                  :aria-label="`编辑 ${r.task.title}`"
                  @click="emit('edit', r.task)"
                >
                  <span
                    class="fill"
                    :style="{
                      width: `${r.task.done ? 100 : r.task.progress}%`,
                    }"
                  /><span class="bar-title">{{
                    r.task.milestone ? "◆" : r.task.title
                  }}</span></button
                ><button
                  v-else
                  class="jump"
                  @click="start = addDays(r.task.start, -2)"
                >
                  跳到 {{ r.task.start }} →
                </button>
              </div>
            </template>
            <template v-else
              ><button
                class="name add-task"
                @click="
                  emit(
                    'create',
                    r.groupId === '__ungrouped__' ? undefined : r.groupId,
                  )
                "
              >
                ＋ 添加任务
              </button>
              <div class="track"
            /></template>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.tools,
.controls {
  display: flex;
  gap: 6px;
  align-items: center;
}
.tools {
  justify-content: space-between;
}
.tools h2 {
  font-size: 22px;
}
.mg button,
.mg select {
  min-height: 44px;
  padding: 8px 12px;
  border: 0;
  border-radius: 10px;
  background: var(--card-fill);
  color: inherit;
}
.tools button {
  background: var(--accent-fill);
  color: var(--text-on-accent);
}
.controls strong {
  flex: 1;
  font-size: 14px;
}
.controls select {
  max-width: 88px;
}
.group-filter {
  margin-top: 10px;
  width: 100%;
}
.hint,
.empty {
  font-size: 12px;
  color: var(--text-secondary);
}
.scroll {
  overflow: auto;
  max-height: 65dvh;
  border: 1px solid var(--divider);
  border-radius: 14px;
  touch-action: pan-x pan-y;
}
.canvas {
  position: relative;
  background: var(--solid-tertiary);
}
.head,
.row {
  display: flex;
}
.head {
  position: sticky;
  top: 0;
  z-index: 5;
  height: 44px;
  background: var(--solid-secondary);
}
.name {
  position: sticky;
  left: 0;
  width: 128px;
  flex: none;
  z-index: 3;
  overflow: hidden;
  background: var(--solid-tertiary);
  padding: 8px 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
  border-right: 1px solid var(--divider);
  font-size: 12px;
}
.head .name {
  background: var(--solid-secondary);
  padding-top: 12px;
}
.ticks {
  display: flex;
  align-items: center;
  font-size: 10px;
  text-align: center;
}
.ticks span {
  flex: none;
}
.rows {
  position: relative;
}
.row {
  height: 52px;
  border-bottom: 1px solid var(--divider);
}
.row .name {
  border-radius: 0;
}
.name small {
  display: block;
  font-weight: 400;
  color: var(--text-secondary);
  font-size: 10px;
}
.track {
  flex: 1;
  position: relative;
  background: repeating-linear-gradient(
    90deg,
    var(--divider) 0 1px,
    transparent 1px var(--day)
  );
  min-height: 52px;
}
.track .bar {
  position: absolute;
  top: 5px;
  height: 42px;
  min-height: 42px;
  border-radius: 8px;
  background: var(--bar);
  overflow: hidden;
  padding: 0;
  border: 0;
  z-index: 2;
  color: var(--text-primary);
}
.bar-title {
  position: relative;
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding: 0 6px;
  font-size: 11px;
  color: #fff;
  text-shadow: 0 1px 2px #000;
}
.fill {
  position: absolute;
  inset: 0 auto 0 0;
  background: #0003;
}
.bar.milestone {
  background: transparent;
  color: var(--bar);
  overflow: visible;
}
.milestone .bar-title {
  color: var(--bar);
  text-shadow: none;
  font-size: 26px;
  padding: 0;
}
.milestone .fill {
  display: none;
}
.group .name {
  background: var(--solid-secondary);
}
.summary {
  position: absolute;
  top: 21px;
  height: 8px;
  border-radius: 3px;
  background: var(--text-tertiary);
}
.add-task {
  color: var(--accent-text) !important;
}
.today-line {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 2px;
  background: var(--accent-fill);
  opacity: 0.5;
  z-index: 1;
  pointer-events: none;
}
.dependencies {
  position: absolute;
  left: 128px;
  top: 0;
  pointer-events: none;
  color: var(--text-tertiary);
  z-index: 1;
}
.track .jump {
  position: sticky;
  left: 136px;
  color: var(--accent-text);
  font-size: 11px;
  background: transparent;
}
</style>
