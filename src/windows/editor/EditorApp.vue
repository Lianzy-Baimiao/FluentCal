<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { confirm, FButton, FCheckbox, FColorSwatches, FDatePicker, FSelect, FSlider, FTextBox, FTimePicker, TASK_COLORS } from '@/fluent';
import type { Task } from '@/types';
import { call, currentWindow, isTauri, on } from '@/api';
import { doc, groupList, initStore } from '@/stores/doc';
import { startThemeSync } from '@/composables/theme';
import { addDays, diffDays, today } from '@/lib/date';
import { dayInfo } from '@/lib/lunar';
import { deleteOccurrence, deleteTask, makeGroup, makeTask, saveGroup, saveTask } from '@/lib/tasks';
import { mergeEdit } from '@/lib/merge';
import RepeatEditor from './RepeatEditor.vue';
import ReminderEditor from './ReminderEditor.vue';
import DepsEditor from './DepsEditor.vue';
import '../window-shell.css';
import './editor.css';

type Payload = { mode: 'edit'; taskId: string; date?: string } | { mode: 'new'; start?: string; end?: string; groupId?: string; title?: string; milestone?: boolean };

startThemeSync();
const draft = ref<Task | null>(null);
const isNew = ref(false);
const occDate = ref('');
const baseline = ref('');
/** The task as it was when the editor loaded it, for the three-way merge on save. */
let base: Task | null = null;
const missing = ref(false);
const titleError = ref(false);
const titleBox = ref<InstanceType<typeof FTextBox>>();
let closing = false;
const dirty = computed(() => !!draft.value && JSON.stringify(draft.value) !== baseline.value);
const clone = <T,>(v: T): T => JSON.parse(JSON.stringify(v));

function load(p: Payload | null) {
  missing.value = false;
  titleError.value = false;
  if (p?.mode === 'edit') {
    const t = doc.tasks[p.taskId];
    draft.value = t ? clone(t) : null;
    missing.value = !t;
    isNew.value = false;
    occDate.value = p.date ?? t?.start ?? '';
  } else {
    const start = p?.start ?? today();
    draft.value = makeTask({ title: p?.title ?? '', start, end: p?.end ?? start, groupId: p?.groupId, milestone: p?.milestone || undefined });
    isNew.value = true;
    occDate.value = start;
  }
  baseline.value = JSON.stringify(draft.value);
  base = draft.value ? clone(draft.value) : null;
  void currentWindow.setTitle(isNew.value ? '新建任务' : '编辑任务');
  void nextTick(() => titleBox.value?.focus());
}

const discardOk = async () => !dirty.value || (await confirm({ title: '放弃未保存的修改？', primaryText: '放弃', closeText: '继续编辑' })) === 'primary';
async function takePending() {
  const p = await call<Payload | null>('pending_take');
  if (p && (await discardOk())) load(p);
}
function close() {
  closing = true;
  void currentWindow.close();
}
async function cancel() {
  if (await discardOk()) close();
}

async function save() {
  const t = draft.value;
  if (!t) return;
  t.title = t.title.trim();
  if (!t.title) {
    titleError.value = true;
    titleBox.value?.focus();
    return;
  }
  if (t.milestone || t.end < t.start) t.end = t.start;
  if (!isNew.value) {
    const current = doc.tasks[t.id];
    if (!current) {
      const r = await confirm({ title: '任务已在其他窗口删除', content: '要用当前内容重新创建它吗？', primaryText: '重新创建', closeText: '放弃' });
      if (r === 'primary') await saveTask(clone(t));
      close();
      return;
    }
    // Only fields changed in this window overwrite the stored task.
    await saveTask(base ? mergeEdit(base, clone(t), current) : clone(t));
  } else {
    await saveTask(clone(t));
  }
  close();
}

async function remove() {
  // Delete the stored task, not the draft, so unsaved edits are not written by "仅这一次".
  const t = draft.value && doc.tasks[draft.value.id];
  if (!t || isNew.value) return close();
  if (t.repeat) {
    const r = await confirm({ title: '删除重复任务', content: `“${t.title}”是重复任务，要删除哪些？`, primaryText: '全部删除', secondaryText: '仅这一次', danger: true });
    if (r === 'close') return;
    await deleteOccurrence(t, occDate.value, r === 'primary' ? 'all' : 'this');
  } else {
    if ((await confirm({ title: '删除任务？', content: `“${t.title}”将被永久删除。`, primaryText: '删除', danger: true })) !== 'primary') return;
    await deleteTask(t.id);
  }
  close();
}

const start = computed({
  get: () => draft.value?.start ?? '',
  set: (v: string) => {
    const t = draft.value;
    if (!t || !v) return;
    const len = Math.max(0, diffDays(t.start, t.end));
    t.start = v;
    t.end = addDays(v, len);
  },
});
const end = computed({ get: () => draft.value?.end ?? '', set: (v: string) => draft.value && v && (draft.value.end = v < draft.value.start ? draft.value.start : v) });
const allDay = computed({ get: () => !draft.value?.time, set: (v: boolean) => draft.value && (draft.value.time = v ? undefined : '09:00') });
const time = computed({ get: () => draft.value?.time ?? '', set: (v: string) => draft.value && (draft.value.time = v || undefined) });
const notes = computed({ get: () => draft.value?.notes ?? '', set: (v: string) => draft.value && (draft.value.notes = v || undefined) });
const color = computed({ get: () => draft.value?.color ?? '', set: (v: string) => draft.value && (draft.value.color = v || undefined) });
const colors = TASK_COLORS.map((c) => ({ key: c.key, color: c.bar, label: c.label }));
const newGroupOpen = ref(false);
const newGroupName = ref('');
const groupOptions = computed(() => [{ value: '', label: '未分组' }, ...groupList.value.map((g) => ({ value: g.id, label: g.name })), { value: '__new__', label: '新建分组…' }]);
const group = computed({
  get: () => draft.value?.groupId ?? '',
  set: (v: string) => (v === '__new__' ? (newGroupOpen.value = true) : draft.value && (draft.value.groupId = v || undefined)),
});
async function createGroup() {
  const name = newGroupName.value.trim();
  if (!name) return;
  const g = makeGroup(name);
  await saveGroup(g);
  if (draft.value) draft.value.groupId = g.id;
  newGroupOpen.value = false;
  newGroupName.value = '';
}

const lunarOf = (iso: string) => dayInfo(iso).text;
const markerOf = (iso: string) => {
  const l = dayInfo(iso).legal;
  return l ? (l.work ? '班' : '休') : undefined;
};
const spanText = computed(() => {
  const t = draft.value;
  if (!t) return '';
  const n = diffDays(t.start, t.end) + 1;
  return `${t.milestone || n === 1 ? '' : `共 ${n} 天 · `}农历${dayInfo(t.start).lunarLabel}`;
});

function onKey(e: KeyboardEvent) {
  if ((e.ctrlKey && e.key === 'Enter') || (e.ctrlKey && e.key.toLowerCase() === 's')) void save();
  else if (e.key === 'Escape' && !document.querySelector('.f-flyout, [role="menu"], [role="dialog"][aria-modal="true"]')) void cancel();
  else return;
  e.preventDefault();
}

onMounted(async () => {
  window.addEventListener('keydown', onKey);
  await initStore();
  load(await call<Payload | null>('pending_take'));
  on('pending://editor', () => void takePending());
  await currentWindow.show();
  await currentWindow.focus();
  if (isTauri) {
    await getCurrentWebviewWindow().onCloseRequested(async (e) => {
      if (closing || !dirty.value) return;
      e.preventDefault();
      if (await discardOk()) close();
    });
  }
});
</script>
<template>
  <div class="aux editor">
    <p v-if="missing" class="missing">这个任务已被删除。</p>
    <template v-else-if="draft">
      <main class="body">
        <FTextBox ref="titleBox" v-model="draft.title" class="title-box" placeholder="任务名称" :maxlength="200" @enter="save" />
        <p v-if="titleError && !draft.title.trim()" class="error" role="alert">请输入任务名称</p>
        <section class="card">
          <div class="row">
            <label>日期</label>
            <div class="ctl">
              <FDatePicker v-model="start" :lunar="lunarOf" :markers="markerOf" :width="150" />
              <template v-if="!draft.milestone">
                <span class="to">至</span>
                <FDatePicker v-model="end" :lunar="lunarOf" :markers="markerOf" :width="150" />
              </template>
              <span class="hint">{{ spanText }}</span>
            </div>
          </div>
          <div class="row">
            <label>时间</label>
            <div class="ctl">
              <FCheckbox v-model="allDay" label="全天" />
              <FTimePicker v-if="!allDay" v-model="time" :width="110" />
            </div>
          </div>
          <div class="row top">
            <label>重复</label>
            <div class="ctl"><RepeatEditor v-model="draft.repeat" :start="draft.start" /></div>
          </div>
          <div class="row top">
            <label>提醒</label>
            <div class="ctl"><ReminderEditor v-model="draft.reminders" :all-day="allDay" :all-day-time="doc.settings.reminders.allDayTime" :enabled="doc.settings.reminders.enabled" /></div>
          </div>
        </section>
        <section class="card">
          <div class="row">
            <label>分组</label>
            <div class="ctl">
              <FSelect v-model="group" :options="groupOptions" :width="200" />
              <template v-if="newGroupOpen">
                <FTextBox v-model="newGroupName" placeholder="分组名称" :width="150" @enter="createGroup" @escape="newGroupOpen = false" />
                <FButton size="small" variant="accent" @click="createGroup">创建</FButton>
              </template>
            </div>
          </div>
          <div class="row">
            <label>颜色</label>
            <div class="ctl"><FColorSwatches v-model="color" :colors="colors" allow-none custom :size="22" /></div>
          </div>
          <div v-if="!draft.repeat" class="row">
            <label>进度</label>
            <div class="ctl">
              <template v-if="!draft.milestone">
                <FSlider v-model="draft.progress" :min="0" :max="100" :step="5" :width="180" show-value :format="(v: number) => `${v}%`" @change="(v: number) => draft && (draft.done = v >= 100)" />
              </template>
              <FCheckbox v-model="draft.done" label="已完成" @update:model-value="(v: boolean) => draft && (draft.progress = v ? 100 : draft.progress === 100 ? 0 : draft.progress)" />
            </div>
          </div>
          <div class="row">
            <label>里程碑</label>
            <div class="ctl">
              <FCheckbox :model-value="!!draft.milestone" label="在甘特图中显示为菱形" @update:model-value="(v: boolean) => draft && (draft.milestone = v || undefined)" />
            </div>
          </div>
          <div class="row top">
            <label>前置任务</label>
            <div class="ctl"><DepsEditor v-model="draft.deps" :self="draft.id" :group-id="draft.groupId" /></div>
          </div>
          <div class="row">
            <label>倒数日</label>
            <div class="ctl">
              <FCheckbox :model-value="!!draft.countdown" label="在日历底部显示“距离这一天还有 N 天”" @update:model-value="(v: boolean) => draft && (draft.countdown = v || undefined)" />
            </div>
          </div>
          <div v-if="!draft.repeat && draft.start === draft.end" class="row">
            <label>顺延</label>
            <div class="ctl">
              <FCheckbox :model-value="draft.autoPostpone ?? doc.settings.tasks.autoPostpone" label="未完成时自动顺延到今天" @update:model-value="(v: boolean) => draft && (draft.autoPostpone = v)" />
            </div>
          </div>
        </section>
        <section class="card notes">
          <FTextBox v-model="notes" multiline :rows="4" placeholder="备注" :maxlength="5000" />
        </section>
      </main>
      <footer class="footer">
        <FButton v-if="!isNew" variant="subtle" icon="delete" class="danger" @click="remove">删除</FButton>
        <span class="grow" />
        <FButton @click="cancel">取消</FButton>
        <FButton variant="accent" :disabled="!draft.title.trim()" @click="save">{{ isNew ? '创建' : '保存' }}</FButton>
      </footer>
    </template>
  </div>
</template>
