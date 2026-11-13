<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { TASK_COLORS, taskColor } from "@/fluent";
import type { ISODate, Task } from "@/types";
import { diffDays, isISODate, today } from "@/lib/date";
import {
  deleteOccurrence,
  deleteTask,
  makeGroup,
  makeTask,
  saveGroup,
  saveTask,
} from "@/lib/tasks";
import { doc, groupList } from "@/stores/doc";
import { mergeEdit } from "@/lib/merge";
import { describeRule, occurrenceOn } from "@/lib/recur";

const props = defineProps<{
  open: boolean;
  task?: Task;
  date?: ISODate;
  groupId?: string;
}>();
const emit = defineEmits<{ close: []; saved: [message: string] }>();

const draft = ref<Task | null>(null);
const isNew = computed(() => !props.task);
const title = ref("");
const start = ref(today());
const end = ref(today());
const time = ref("");
const notes = ref("");
const color = ref("");
const groupId = ref("");
const progress = ref(0);
const done = ref(false);
const repeat = ref("");
const reminder = ref("");
const milestone = ref(false);
const error = ref("");
const busy = ref(false);
const deleting = ref(false);
const discard = ref(false);
const panel = ref<HTMLElement>();
let baseline = "";
let restoreFocus: HTMLElement | null = null;
const newGroupName = ref("");
const showNewGroup = ref(false);
const state = () =>
  JSON.stringify([
    title.value,
    start.value,
    end.value,
    time.value,
    notes.value,
    color.value,
    groupId.value,
    progress.value,
    done.value,
    repeat.value,
    reminder.value,
    milestone.value,
  ]);

function load() {
  const t = props.task;
  draft.value = t ? JSON.parse(JSON.stringify(t)) : null;
  title.value = t?.title ?? "";
  start.value = t?.start ?? props.date ?? today();
  end.value = t?.end ?? props.date ?? today();
  time.value = t?.time ?? "";
  notes.value = t?.notes ?? "";
  color.value = t?.color ?? "";
  groupId.value = t?.groupId ?? props.groupId ?? "";
  progress.value = t?.progress ?? 0;
  done.value =
    t?.repeat && props.date ? !!occurrenceOn(t, props.date)?.done : !!t?.done;
  repeat.value = t?.repeat?.freq ?? "";
  reminder.value = JSON.stringify(t?.reminders ?? []);
  milestone.value = !!t?.milestone;
  error.value = "";
  deleting.value = discard.value = showNewGroup.value = false;
  newGroupName.value = "";
  baseline = state();
}
watch(
  () => props.open,
  async (v) => {
    if (v) {
      restoreFocus = document.activeElement as HTMLElement;
      load();
      await nextTick();
      panel.value?.focus();
    } else restoreFocus?.focus();
  },
  { immediate: true },
);
function cancel() {
  if (busy.value) return;
  if (state() !== baseline) discard.value = true;
  else emit("close");
}
defineExpose({ cancel });
function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    cancel();
  }
  if (e.key !== "Tab") return;
  const els = [
    ...(panel.value?.querySelectorAll<HTMLElement>(
      "button:not(:disabled), input, select, textarea",
    ) ?? []),
  ].filter((el) => el.offsetParent !== null);
  const index = els.indexOf(document.activeElement as HTMLElement);
  if (e.shiftKey && index <= 0) {
    e.preventDefault();
    els.at(-1)?.focus();
  } else if (!e.shiftKey && (index < 0 || index === els.length - 1)) {
    e.preventDefault();
    els[0]?.focus();
  }
}

const colors = TASK_COLORS.slice(0, 8);
const groups = computed(() => groupList.value);
const dependencyNames = computed(() =>
  draft.value?.deps?.map((id) => doc.tasks[id]?.title ?? "已删除").join("、"),
);
const duration = computed(() => Math.max(0, diffDays(start.value, end.value)));
function normalizeDates() {
  if (!start.value) start.value = today();
  if (!end.value || end.value < start.value) end.value = start.value;
}

async function addGroup() {
  const name = newGroupName.value.trim();
  if (!name) return;
  const g = makeGroup(name);
  try {
    await saveGroup(g);
    groupId.value = g.id;
    newGroupName.value = "";
    showNewGroup.value = false;
  } catch (e) {
    error.value = `分组保存失败：${e}`;
  }
}

async function save() {
  if (busy.value) return;
  if (
    !isISODate(start.value) ||
    !isISODate(end.value) ||
    start.value < "1901-01-01" ||
    end.value > "2099-12-31" ||
    end.value < start.value
  ) {
    error.value = "请填写有效的起止日期（1901—2099 年），结束不能早于开始";
    return;
  }
  const name = title.value.trim();
  if (!name) {
    error.value = "请输入任务名称";
    return;
  }
  const base = draft.value;
  const next: Task = base
    ? {
        ...base,
        title: name,
        start: start.value,
        end: end.value,
        time: time.value || undefined,
        notes: notes.value.trim() || undefined,
        color: color.value || undefined,
        groupId: groupId.value || undefined,
        progress: done.value ? 100 : progress.value,
        done: done.value || progress.value >= 100,
        repeat: repeat.value
          ? repeat.value === base.repeat?.freq
            ? base.repeat
            : {
                freq: repeat.value as NonNullable<Task["repeat"]>["freq"],
                interval: 1,
              }
          : undefined,
        updatedAt: Date.now(),
      }
    : makeTask({
        title: name,
        start: start.value,
        end: end.value,
        time: time.value || undefined,
        notes: notes.value.trim() || undefined,
        color: color.value || undefined,
        groupId: groupId.value || undefined,
        progress: done.value ? 100 : progress.value,
        done: done.value || progress.value >= 100,
        repeat: repeat.value
          ? {
              freq: repeat.value as NonNullable<Task["repeat"]>["freq"],
              interval: 1,
            }
          : undefined,
      });
  next.reminders = JSON.parse(reminder.value || "[]");
  next.milestone = milestone.value || undefined;
  if (next.milestone) next.end = next.start;
  if (next.repeat) {
    next.done = false;
    next.progress = 0;
    const dates = new Set(base?.doneDates ?? []);
    const date = props.date || next.start;
    if (done.value) dates.add(date);
    else dates.delete(date);
    next.doneDates = [...dates].sort();
  } else {
    next.doneDates = undefined;
    next.exDates = undefined;
  }
  busy.value = true;
  try {
    if (base && !doc.tasks[base.id])
      throw new Error("这个任务已经被删除，请关闭编辑器后重新创建");
    await saveTask(base ? mergeEdit(base, next, doc.tasks[base.id]) : next);
    emit("saved", "任务已保存");
    emit("close");
  } catch (e) {
    error.value = `保存失败：${e}`;
  } finally {
    busy.value = false;
  }
}

async function remove(all = false) {
  const t = draft.value;
  if (!t) return emit("close");
  busy.value = true;
  try {
    if (t.repeat && props.date && !all)
      await deleteOccurrence(t, props.date, "this");
    else await deleteTask(t.id);
    emit("saved", "任务已删除，可用顶部撤销按钮恢复");
    emit("close");
  } catch (e) {
    error.value = `删除失败：${e}`;
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Transition name="sheet">
    <div v-if="open" class="sheet-layer" @click.self="cancel">
      <section
        ref="panel"
        class="sheet"
        tabindex="-1"
        role="dialog"
        aria-modal="true"
        :aria-label="isNew ? '新建任务' : '编辑任务'"
        @keydown="onKey"
      >
        <header class="sheet-head">
          <button
            class="icon-btn"
            type="button"
            aria-label="关闭"
            @click="cancel"
          >
            ×
          </button>
          <h2>{{ isNew ? "新建任务" : "编辑任务" }}</h2>
          <button
            class="save-btn"
            type="button"
            :disabled="busy || !title.trim()"
            @click="save"
          >
            {{ busy ? "保存中" : "保存" }}
          </button>
        </header>
        <div class="form-scroll">
          <p v-if="discard" class="confirm-strip">
            放弃未保存的修改？
            <button type="button" @click="emit('close')">放弃</button
            ><button type="button" @click="discard = false">继续编辑</button>
          </p>
          <input
            v-model="title"
            class="title-input"
            maxlength="200"
            placeholder="任务名称"
            aria-label="任务名称"
            @keydown.enter.prevent="save"
          />
          <p v-if="error" class="error" role="alert">{{ error }}</p>
          <div class="form-card">
            <label class="field"
              ><span>开始</span
              ><input
                v-model="start"
                type="date"
                min="1901-01-01"
                max="2099-12-31"
                @change="normalizeDates"
            /></label>
            <label v-if="!milestone" class="field"
              ><span>结束</span
              ><input v-model="end" type="date" :min="start" max="2099-12-31"
            /></label>
            <label class="field"
              ><span>时间</span><input v-model="time" type="time"
            /></label>
            <label class="field"
              ><span>重复</span
              ><select v-model="repeat" aria-label="重复">
                <option value="">不重复</option>
                <option value="daily">每天</option>
                <option value="weekly">每周</option>
                <option value="monthly">每月</option>
                <option value="yearly">每年</option>
                <option value="lunarYearly">每年（农历）</option>
                <option value="workday">工作日</option>
                <option value="legalWorkday">法定工作日</option>
                <option value="monthLastDay">每月最后一天</option>
                <option value="ebbinghaus">艾宾浩斯</option>
              </select></label
            >
            <p
              v-if="draft?.repeat && repeat === draft.repeat.freq"
              class="rule-note"
            >
              {{ describeRule(draft.repeat, draft.start) }}（原规则保留）
            </p>
            <label class="field"
              ><span>提醒</span
              ><select v-model="reminder" aria-label="提醒">
                <option value="[]">不提醒</option>
                <option value="[0]">准时</option>
                <option value="[15]">提前 15 分钟</option>
                <option value="[60]">提前 1 小时</option>
                <option value="[1440]">提前 1 天</option>
                <option
                  v-if="
                    draft?.reminders?.length &&
                    !['[0]', '[15]', '[60]', '[1440]'].includes(
                      JSON.stringify(draft.reminders),
                    )
                  "
                  :value="JSON.stringify(draft.reminders)"
                >
                  保留桌面端提醒
                </option>
              </select></label
            >
            <p class="rule-note">
              全天任务在 09:00 提醒，请先在设置中开启通知权限。
            </p>
          </div>
          <div class="form-card">
            <div class="field">
              <span>颜色</span>
              <div class="swatches">
                <button
                  v-for="c in colors"
                  :key="c.key"
                  type="button"
                  class="swatch"
                  :class="{ selected: color === c.key }"
                  :style="{ background: taskColor(c.key, false).bar }"
                  :aria-label="c.label"
                  @click="color = color === c.key ? '' : c.key"
                />
              </div>
            </div>
            <label class="field"
              ><span>分组</span
              ><select v-model="groupId" aria-label="分组">
                <option value="">未分组</option>
                <option v-for="g in groups" :key="g.id" :value="g.id">
                  {{ g.name }}
                </option>
              </select></label
            >
            <div class="new-group">
              <button
                type="button"
                class="link-btn"
                @click="showNewGroup = !showNewGroup"
              >
                ＋ 新建分组
              </button>
              <div v-if="showNewGroup" class="new-group-row">
                <input
                  v-model="newGroupName"
                  placeholder="分组名称"
                  @keydown.enter="addGroup"
                /><button type="button" @click="addGroup">添加</button>
              </div>
            </div>
          </div>
          <div class="form-card">
            <template v-if="!repeat"
              ><div class="progress-label">
                <span>进度</span><strong>{{ done ? 100 : progress }}%</strong>
              </div>
              <input
                v-model.number="progress"
                aria-label="进度"
                class="range"
                type="range"
                min="0"
                max="100"
                step="5"
                @change="done = progress >= 100"
            /></template>
            <label class="done-row"
              ><input
                v-model="done"
                type="checkbox"
                @change="
                  progress = done ? 100 : progress === 100 ? 0 : progress
                "
              />
              {{ repeat ? "本次已完成" : "已完成" }}</label
            >
            <label class="done-row"
              ><input v-model="milestone" type="checkbox" /> 里程碑</label
            >
            <p v-if="dependencyNames" class="rule-note">
              前置任务：{{ dependencyNames }}
            </p>
          </div>
          <div class="form-card notes-card">
            <textarea
              v-model="notes"
              aria-label="备注"
              rows="4"
              maxlength="5000"
              placeholder="备注"
            />
          </div>
          <button
            v-if="!isNew && !deleting"
            type="button"
            class="delete-btn"
            :disabled="busy"
            @click="deleting = true"
          >
            删除任务
          </button>
          <div v-if="deleting" class="confirm-strip">
            确认删除“{{ draft?.title }}”？<button
              type="button"
              :disabled="busy"
              @click="remove(false)"
            >
              {{ draft?.repeat ? "仅这一次" : "确认删除" }}</button
            ><button
              v-if="draft?.repeat"
              type="button"
              :disabled="busy"
              @click="remove(true)"
            >
              删除整个系列</button
            ><button type="button" @click="deleting = false">取消</button>
          </div>
          <p class="duration-hint">
            {{ duration + 1 }} 天 · 可在月历和甘特图中继续调整
          </p>
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
  max-height: min(92vh, 760px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-radius: 24px 24px 0 0;
  background: var(--solid-base);
  color: var(--text-primary);
  box-shadow: 0 -12px 36px rgba(0, 0, 0, 0.22);
}
.sheet {
  max-height: min(95dvh, 850px);
}
.rule-note {
  margin: 0;
  padding: 10px 14px;
  font-size: 12px;
  color: var(--text-secondary);
}
.confirm-strip {
  padding: 12px;
  border-radius: 12px;
  background: var(--caution-bg);
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.confirm-strip button {
  min-height: 40px;
  border: 0;
  border-radius: 8px;
  padding: 6px 12px;
  background: var(--control-fill);
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
  text-align: center;
}
.icon-btn,
.save-btn,
.link-btn,
.new-group-row button,
.delete-btn {
  border: 0;
  background: transparent;
  color: inherit;
}
.icon-btn {
  width: 32px;
  height: 32px;
  font-size: 27px;
  line-height: 1;
  color: var(--text-secondary);
}
.save-btn {
  padding: 7px 2px;
  color: var(--accent-text);
  font-weight: 650;
}
.save-btn:disabled {
  opacity: 0.4;
}
.form-scroll {
  overflow-y: auto;
  padding: 18px 16px calc(28px + env(safe-area-inset-bottom));
}
.title-input {
  width: 100%;
  padding: 13px 14px;
  border: 1px solid var(--control-stroke-secondary);
  border-radius: 12px;
  outline: 0;
  background: var(--control-fill-input-active);
  font-size: 19px;
  font-weight: 650;
}
.title-input:focus {
  border-color: var(--accent-fill);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent-fill) 26%, transparent);
}
.error {
  margin: 6px 4px 0;
  color: var(--critical);
  font-size: 12px;
}
.form-card {
  margin-top: 12px;
  overflow: hidden;
  border: 1px solid var(--card-stroke);
  border-radius: 14px;
  background: var(--card-fill);
}
.field {
  display: flex;
  align-items: center;
  min-height: 50px;
  gap: 12px;
  padding: 8px 14px;
  border-bottom: 1px solid var(--divider);
}
.field:last-child {
  border-bottom: 0;
}
.field > span {
  width: 48px;
  flex: none;
  color: var(--text-secondary);
  font-size: 13px;
}
.field input,
.field select,
.new-group-row input {
  min-width: 0;
  flex: 1;
  border: 0;
  outline: 0;
  background: transparent;
  color: inherit;
  font-size: 15px;
  text-align: right;
}
.field select {
  appearance: auto;
}
.swatches {
  display: flex;
  flex: 1;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 4px;
}
.swatch {
  width: 26px;
  height: 36px;
  padding: 0;
  border: 5px solid var(--solid-base);
  border-radius: 50%;
}
.swatch.selected {
  border-color: var(--text-primary);
  box-shadow: 0 0 0 2px var(--solid-base);
}
.new-group {
  padding: 8px 14px;
  border-top: 1px solid var(--divider);
}
.link-btn {
  padding: 4px 0;
  color: var(--accent-text);
  font-size: 13px;
}
.new-group-row {
  display: flex;
  gap: 8px;
  padding-top: 6px;
}
.new-group-row input {
  padding: 8px;
  border: 1px solid var(--control-stroke-secondary);
  border-radius: 8px;
  text-align: left;
}
.new-group-row button {
  padding: 0 8px;
  color: var(--accent-text);
}
.progress-label {
  display: flex;
  justify-content: space-between;
  padding: 14px 14px 2px;
  color: var(--text-secondary);
  font-size: 13px;
}
.progress-label strong {
  color: var(--accent-text);
}
.range {
  width: calc(100% - 28px);
  margin: 8px 14px 4px;
  accent-color: var(--accent-fill);
}
.done-row {
  display: flex;
  gap: 8px;
  align-items: center;
  padding: 8px 14px 14px;
  font-size: 13px;
}
.notes-card {
  padding: 11px 14px;
}
.notes-card textarea {
  width: 100%;
  resize: vertical;
  border: 0;
  outline: 0;
  background: transparent;
  color: inherit;
  line-height: 1.5;
}
.delete-btn {
  width: 100%;
  margin-top: 14px;
  padding: 13px;
  border-radius: 12px;
  background: color-mix(in srgb, var(--critical) 12%, transparent);
  color: var(--critical);
  font-weight: 600;
}
.duration-hint {
  margin: 12px 0 0;
  color: var(--text-tertiary);
  text-align: center;
  font-size: 12px;
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
    width: min(560px, 94vw);
    max-height: 88vh;
    border-radius: 22px;
  }
}
</style>
