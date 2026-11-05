<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Capacitor, type PluginListenerHandle } from "@capacitor/core";
import { App } from "@capacitor/app";
import { LocalNotifications } from "@capacitor/local-notifications";
import { FIcon, taskColor } from "@/fluent";
import type { Doc, ISODate, Task } from "@/types";
import {
  addDays,
  addMonths,
  formatFriendly,
  monthWeeks,
  parts,
  startOfMonth,
  weekday,
  WEEKDAY_SHORT,
} from "@/lib/date";
import { dayInfo } from "@/lib/lunar";
import { agenda, dayHeading } from "@/lib/agenda";
import { occurrences, type Occurrence } from "@/lib/recur";
import {
  doc,
  flushSettings,
  history,
  initStore,
  patchSettings,
  put,
  storeState,
  taskList,
  undo,
  replaceDoc,
  exportDoc,
} from "@/stores/doc";
import { clearSampleData, setOccurrenceDone } from "@/lib/tasks";
import { sampleData } from "@/lib/seed";
import { planMerge, toCSV, toICS } from "@/lib/export";
import { readDocument } from "@/mobile/data";
import { updateNotifications } from "@/mobile/notifications";
import { call } from "@/api";
import { now, startClock } from "@/composables/now";
import { startThemeSync, isDark } from "@/composables/theme";
import MobileEditor from "./MobileEditor.vue";
import MobileGantt from "./MobileGantt.vue";
import MobileSync from "./MobileSync.vue";
import MobileUpdates from "./MobileUpdates.vue";
import { startSync, resumeSync, syncStatus } from "@/mobile/sync";

type Tab = "agenda" | "calendar" | "gantt" | "settings";
const tab = ref<Tab>("agenda");
const cursor = ref(startOfMonth(now.value));
const selected = ref<ISODate>(now.value);
const editorOpen = ref(false);
const editorTask = ref<Task>();
const editorDate = ref<ISODate>(now.value);
const editorGroup = ref("");
const editorRef = ref<InstanceType<typeof MobileEditor>>();
const search = ref("");
const showSearch = ref(false);
const toast = ref("");
const error = ref("");
const loading = ref(true);
const importing = ref(false);
const pendingImport = ref<Doc>();
const importInput = ref<HTMLInputElement>();
const handles: PluginListenerHandle[] = [];
let toastTimer: ReturnType<typeof setTimeout> | undefined;
let notificationTimer: ReturnType<typeof setTimeout> | undefined;
let disposed = false;
let stopSync: (() => void) | undefined;

const activeTitle = computed(
  () =>
    ({
      agenda: "近期日程",
      calendar: parts(cursor.value)[0] + "年" + parts(cursor.value)[1] + "月",
      gantt: "项目甘特图",
      settings: "设置",
    })[tab.value],
);
const taskCount = computed(
  () =>
    taskList.value.filter((t) =>
      t.repeat
        ? occurrences(t, now.value, now.value, 10).some((o) => !o.done)
        : !t.done,
    ).length,
);
const hasSamples = computed(() => taskList.value.some((t) => t.sample));
const filteredTasks = computed(() => {
  const q = search.value.trim().toLowerCase();
  return taskList.value.filter(
    (t) => !q || (t.title + " " + (t.notes ?? "")).toLowerCase().includes(q),
  );
});
const agendaDays = computed(() =>
  search.value.trim()
    ? [
        {
          date: now.value,
          items: filteredTasks.value.map((t) => ({
            task: t,
            date: t.start,
            start: t.start,
            end: t.end,
            done: t.done,
          })),
        },
      ]
    : agenda(
        filteredTasks.value,
        now.value,
        addDays(now.value, 29),
        now.value,
        doc.settings.tasks.showCompleted,
      ),
);
const selectedItems = computed(
  () =>
    agenda(
      filteredTasks.value,
      selected.value,
      selected.value,
      selected.value,
      true,
    )[0]?.items ?? [],
);
const weeks = computed(() =>
  monthWeeks(
    parts(cursor.value)[0],
    parts(cursor.value)[1],
    doc.settings.calendar.weekStart,
    "six",
  ),
);
const monthCells = computed(() =>
  weeks.value.flatMap((w) =>
    Array.from({ length: 7 }, (_, i) => addDays(w, i)),
  ),
);
const monthOccs = computed(() =>
  filteredTasks.value.flatMap((t) =>
    occurrences(t, monthCells.value[0], monthCells.value.at(-1)!, 200),
  ),
);
const monthTaskCount = (d: ISODate) =>
  monthOccs.value.filter((o) => o.start <= d && o.end >= d).length;
const mergePlan = computed(() =>
  pendingImport.value ? planMerge(doc, pendingImport.value) : null,
);

function notify(text: string) {
  toast.value = text;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (toast.value = ""), 4000);
}
async function action(fn: () => Promise<unknown>) {
  try {
    await fn();
  } catch (e) {
    error.value = String(e);
  }
}
function openNew(date = selected.value, groupId = "") {
  editorTask.value = undefined;
  editorDate.value = date;
  editorGroup.value = groupId;
  editorOpen.value = true;
}
function openTask(t: Task, date = t.start) {
  editorTask.value = t;
  editorDate.value = date;
  editorOpen.value = true;
}
function closeEditor() {
  editorOpen.value = false;
  editorTask.value = undefined;
}
function nextMonth(delta: number) {
  const next = addMonths(cursor.value, delta);
  if (next >= "1901-02-01" && next <= "2099-11-01") cursor.value = next;
}
function goToday() {
  cursor.value = startOfMonth(now.value);
  selected.value = now.value;
}
function pickDay(d: ISODate) {
  selected.value = d;
  if (d.slice(0, 7) !== cursor.value.slice(0, 7))
    cursor.value = startOfMonth(d);
}
function occurrenceDone(o: Occurrence) {
  void action(() => setOccurrenceDone(o.task, o.date, !o.done));
}
function displayTime(t: Task) {
  return t.time || "全天";
}
function groupName(t: Task) {
  return t.groupId ? doc.groups[t.groupId]?.name : "";
}
function percent(t: Task) {
  return t.done ? 100 : Math.max(0, Math.min(100, t.progress));
}
function undoLast() {
  void action(async () => {
    await undo();
    notify("已撤销");
  });
}

async function seed() {
  if (doc.settings.general.seeded) return;
  if (!Object.keys(doc.tasks).length && !Object.keys(doc.groups).length) {
    const data = sampleData(now.value);
    await put("groups", Object.fromEntries(data.groups.map((g) => [g.id, g])), {
      history: false,
    });
    await put("tasks", Object.fromEntries(data.tasks.map((t) => [t.id, t])), {
      history: false,
    });
  }
  patchSettings({ general: { seeded: true } });
  await flushSettings();
}
async function exportFile(kind: "json" | "csv" | "ics") {
  await action(async () => {
    await flushSettings();
    const content =
      kind === "json"
        ? JSON.stringify((await call<{ doc: Doc }>("store_get")).doc, null, 2)
        : kind === "csv"
          ? toCSV(taskList.value, doc.groups)
          : toICS(taskList.value, doc.groups);
    const name = "fluentcal-" + now.value + "." + kind;
    await call("file_save", {
      content,
      defaultName: name,
      bom: kind === "csv",
    });
    notify(Capacitor.isNativePlatform() ? "已打开系统分享面板" : "备份已下载");
  });
}
async function importFile(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  await action(async () => {
    if (file.size > 20_000_000) throw new Error("备份超过 20 MB");
    pendingImport.value = readDocument(await file.text());
  });
  input.value = "";
}
async function confirmImport() {
  if (!pendingImport.value || !mergePlan.value || importing.value) return;
  importing.value = true;
  await action(async () => {
    await flushSettings();
    const plan = mergePlan.value!;
    const next = exportDoc();
    Object.assign(next.tasks, plan.tasks);
    Object.assign(next.groups, plan.groups);
    Object.assign(next.dayMarks, plan.dayMarks);
    Object.assign(next.holidays, pendingImport.value!.holidays);
    await replaceDoc(next);
    pendingImport.value = undefined;
    notify(
      "已合并备份：新增 " + plan.added + " 项，更新 " + plan.updated + " 项",
    );
  });
  importing.value = false;
}
async function clearSamples() {
  await action(async () => {
    const n = await clearSampleData();
    patchSettings({ general: { onboarded: true } });
    await flushSettings();
    notify("已清除 " + n + " 个示例任务，可撤销");
  });
}
async function requestNotifications() {
  if (!Capacitor.isNativePlatform())
    return notify("请在安卓客户端中开启通知权限");
  await action(async () => {
    const p = await LocalNotifications.requestPermissions();
    if (p.display !== "granted")
      return notify("未获得通知权限，请在 Android 系统设置中允许通知");
    patchSettings({ reminders: { enabled: true, allDayTime: "09:00" } });
    await flushSettings();
    await updateNotifications(doc);
    notify("提醒已开启");
  });
}
function scheduleNotifications() {
  clearTimeout(notificationTimer);
  notificationTimer = setTimeout(() => {
    if (storeState.ready) void action(() => updateNotifications(doc));
  }, 500);
}
async function setTheme(e: Event) {
  patchSettings({
    theme: {
      mode: (e.target as HTMLSelectElement).value as
        | "system"
        | "light"
        | "dark",
    },
  });
  await action(flushSettings);
}
function back() {
  if (editorOpen.value) editorRef.value?.cancel();
  else if (pendingImport.value && !importing.value)
    pendingImport.value = undefined;
  else if (showSearch.value) {
    showSearch.value = false;
    search.value = "";
  } else if (tab.value !== "agenda") tab.value = "agenda";
  else void App.minimizeApp();
}
function onUnhandled(e: PromiseRejectionEvent) {
  error.value = "操作失败：" + String(e.reason);
  e.preventDefault();
}
watch([() => doc.tasks, () => doc.settings.reminders], scheduleNotifications, {
  deep: true,
});
onMounted(async () => {
  window.addEventListener("unhandledrejection", onUnhandled);
  try {
    startThemeSync();
    await initStore();
    await seed();
    stopSync = await startSync();
    startClock();
    if (storeState.recovered)
      error.value = "数据已从上一次保存恢复，请导出备份。";
    scheduleNotifications();
    if (Capacitor.isNativePlatform()) {
      handles.push(await App.addListener("backButton", back));
      handles.push(
        await App.addListener("appStateChange", ({ isActive }) => {
          if (isActive) { scheduleNotifications(); void resumeSync(); }
          else void flushSettings().then(() => resumeSync());
        }),
      );
      handles.push(
        await LocalNotifications.addListener(
          "localNotificationActionPerformed",
          (event) => {
            const extra = event.notification.extra;
            const task = doc.tasks[extra?.taskId];
            if (task) openTask(task, extra.date);
          },
        ),
      );
      if (disposed) handles.forEach((h) => void h.remove());
    }
  } catch (e) {
    error.value = "启动失败：" + e;
  } finally {
    loading.value = false;
  }
});
onBeforeUnmount(() => {
  disposed = true;
  stopSync?.();
  handles.forEach((h) => void h.remove());
  clearTimeout(toastTimer);
  clearTimeout(notificationTimer);
  window.removeEventListener("unhandledrejection", onUnhandled);
});
</script>

<template>
  <div class="mobile-app">
    <div class="app-main" :inert="editorOpen || !!pendingImport">
      <header class="app-bar">
        <div class="brand">
          <span class="brand-mark">F</span>
          <div>
            <strong>FluentCal</strong><small>{{ activeTitle }}<span v-if="syncStatus.enabled" class="sync-indicator" :class="{ failed: !!syncStatus.error }" :title="syncStatus.error || '自动云同步已开启'"> · {{ syncStatus.syncing ? '同步中' : syncStatus.error ? '同步待处理' : syncStatus.pending ? '待同步' : '云端已同步' }}</span></small>
          </div>
        </div>
        <div class="bar-actions">
          <button
            v-if="history.canUndo"
            type="button"
            class="bar-btn"
            aria-label="撤销"
            @click="undoLast"
          >
            <FIcon name="undo" :size="19" />
          </button>
          <button
            v-if="tab !== 'settings'"
            type="button"
            class="bar-btn"
            aria-label="搜索"
            @click="showSearch = !showSearch"
          >
            <FIcon name="search" :size="19" />
          </button>
          <button
            type="button"
            class="bar-btn"
            aria-label="今天"
            @click="goToday"
          >
            <FIcon name="today" :size="19" />
          </button>
        </div>
      </header>
      <div v-if="showSearch" class="search-row">
        <FIcon name="search" :size="16" /><input
          v-model="search"
          autofocus
          placeholder="搜索任务、备注…"
        /><button
          type="button"
          @click="
            search = '';
            showSearch = false;
          "
        >
          ×
        </button>
      </div>
      <p v-if="error" class="error-banner" role="alert">
        {{ error }}<button @click="error = ''">关闭</button>
      </p>
      <p v-if="loading" class="loading">正在载入本机日历…</p>
      <main v-if="storeState.ready" class="content">
        <section v-if="tab === 'agenda'" class="agenda-page">
          <div v-if="hasSamples" class="sample-notice">
            当前包含示例项目，可随时清除。<button @click="clearSamples">
              清除示例
            </button>
          </div>
          <div class="hero">
            <div>
              <p class="eyebrow">未来 30 天</p>
              <h1>{{ taskCount }} <span>项待完成</span></h1>
            </div>
            <button type="button" class="primary-round" @click="openNew()">
              ＋
            </button>
          </div>

          <div
            v-if="!agendaDays.some((d) => d.items.length)"
            class="empty-state"
          >
            <div class="empty-icon">✓</div>
            <h2>{{ search ? "没有找到任务" : "近期没有安排" }}</h2>
            <p>享受轻松的一天，或添加一个新任务</p>
            <button type="button" class="primary-btn" @click="openNew()">
              添加任务
            </button>
          </div>
          <section v-for="d in agendaDays" :key="d.date" class="day-section">
            <div class="day-heading">
              <div>
                <strong :class="{ today: d.date === now }">{{
                  search
                    ? "搜索结果 · " + d.items.length + " 项"
                    : dayHeading(d.date, now)
                }}</strong
                ><span>{{ formatFriendly(d.date).split(" ")[1] }}</span>
              </div>
              <button type="button" @click="openNew(d.date)">＋</button>
            </div>
            <div
              v-for="o in d.items"
              :key="`${o.task.id}@${o.date}`"
              type="button"
              class="task-card"
              :class="{ done: o.done }"
            >
              <span
                class="task-accent"
                :style="{ background: taskColor(o.task.color, isDark).bar }"
              /><button
                type="button"
                class="check"
                :class="{ checked: o.done }"
                :aria-label="(o.done ? '取消完成 ' : '完成 ') + o.task.title"
                :aria-pressed="o.done"
                @click.stop="occurrenceDone(o)"
              >
                {{ o.done ? "✓" : "" }}</button
              ><button
                type="button"
                class="task-main"
                @click="openTask(o.task, o.date)"
              >
                <strong>{{ o.task.title }}</strong
                ><span class="task-meta"
                  ><span>{{ displayTime(o.task) }}</span
                  ><span v-if="groupName(o.task)"
                    >· {{ groupName(o.task) }}</span
                  ><span v-if="o.task.repeat">· 重复</span></span
                ><span
                  v-if="!o.task.repeat && percent(o.task) > 0"
                  class="mini-progress"
                  ><i
                    :style="{
                      width: `${percent(o.task)}%`,
                      background: taskColor(o.task.color, isDark).bar,
                    }"
                /></span></button
              ><FIcon name="chevronRight" :size="16" class="chevron" />
            </div>
          </section>
          <div v-if="agendaDays.length" class="agenda-more">
            <button type="button" @click="tab = 'calendar'">
              查看月历 <FIcon name="chevronRight" :size="14" />
            </button>
          </div>
        </section>
        <section v-else-if="tab === 'calendar'" class="calendar-page">
          <div class="calendar-toolbar">
            <button type="button" class="nav-btn" @click="nextMonth(-1)">
              ‹
            </button>
            <h2>{{ activeTitle }}</h2>
            <button type="button" class="nav-btn" @click="nextMonth(1)">
              ›</button
            ><button type="button" class="today-btn" @click="goToday">
              今天
            </button>
          </div>
          <div class="weekday-row">
            <span
              v-for="(_, i) in 7"
              :key="i"
              :class="{
                rest: doc.settings.calendar.restDays.includes(
                  (doc.settings.calendar.weekStart + i) % 7,
                ),
              }"
              >{{
                WEEKDAY_SHORT[(doc.settings.calendar.weekStart + i) % 7]
              }}</span
            >
          </div>
          <div class="month-grid">
            <button
              v-for="d in monthCells"
              :key="d"
              type="button"
              class="month-day"
              :aria-label="
                d +
                '，' +
                dayInfo(d).lunarLabel +
                '，' +
                monthTaskCount(d) +
                ' 项任务'
              "
              :aria-pressed="d === selected"
              :class="{
                dim: d.slice(0, 7) !== cursor.slice(0, 7),
                today: d === now,
                selected: d === selected,
                rest: doc.settings.calendar.restDays.includes(weekday(d)),
              }"
              @click="pickDay(d)"
            >
              <b>{{ parts(d)[2] }}</b
              ><small>{{ dayInfo(d).text }}</small
              ><i
                v-if="monthTaskCount(d)"
                :style="{
                  background: taskColor(
                    monthOccs.find((o) => o.start <= d && o.end >= d)?.task
                      .color,
                    isDark,
                  ).bar,
                }"
              />
            </button>
          </div>
          <div class="selected-day">
            <div class="selected-day-head">
              <h3>
                {{ selected === now ? "今天" : formatFriendly(selected) }}
              </h3>
              <button
                type="button"
                class="primary-btn small"
                @click="openNew(selected)"
              >
                添加任务
              </button>
            </div>
            <p v-if="!selectedItems.length" class="muted">这一天还没有任务</p>
            <button
              v-for="o in selectedItems"
              :key="`${o.task.id}@${o.date}`"
              type="button"
              class="compact-task"
              @click="openTask(o.task, o.date)"
            >
              <span
                class="task-accent"
                :style="{ background: taskColor(o.task.color, isDark).bar }"
              /><span>{{ o.task.title }}</span
              ><small>{{ displayTime(o.task) }}</small>
            </button>
          </div>
        </section>
        <MobileGantt
          v-else-if="tab === 'gantt'"
          :search="search"
          @edit="openTask"
          @create="openNew(selected, $event)"
        />
        <section v-else class="settings-page">
          <div class="settings-profile">
            <span class="profile-icon">F</span>
            <div>
              <h2>FluentCal</h2>
              <p>安卓客户端 · 离线可用</p>
            </div>
          </div>
          <div class="settings-card">
            <h3>外观</h3>
            <label class="setting-row"
              ><span>主题</span
              ><select aria-label="主题" :value="doc.settings.theme.mode" @change="setTheme">
                <option value="system">跟随系统</option>
                <option value="light">浅色</option>
                <option value="dark">深色</option>
              </select></label
            ><button
              type="button"
              class="setting-row action"
              @click="requestNotifications"
            >
              <span>系统提醒</span><small>开启通知权限</small
              ><FIcon name="chevronRight" :size="16" />
            </button>
          </div>
          <MobileSync />
          <div class="settings-card">
            <h3>数据管理</h3>
            <button
              type="button"
              class="setting-row action"
              @click="exportFile('json')"
            >
              <span>导出备份</span><small>JSON</small
              ><FIcon name="download" :size="16" /></button
            ><button
              type="button"
              class="setting-row action"
              @click="importInput?.click()"
            >
              <span>导入备份</span><small>合并桌面版 JSON</small
              ><FIcon name="chevronRight" :size="16" /></button
            ><button
              type="button"
              class="setting-row action"
              @click="exportFile('csv')"
            >
              <span>导出任务表</span><small>CSV</small
              ><FIcon name="download" :size="16" /></button
            ><button
              type="button"
              class="setting-row action"
              @click="exportFile('ics')"
            >
              <span>导出日历</span><small>ICS</small
              ><FIcon name="download" :size="16" /></button
            ><button
              type="button"
              class="setting-row danger"
              @click="clearSamples"
            >
              <span>清除示例数据</span
              ><FIcon name="chevronRight" :size="16" /></button
            ><input
              ref="importInput"
              class="hidden"
              type="file"
              accept=".json,application/json"
              @change="importFile"
            />
          </div>
          <MobileUpdates />
          <p class="data-note">
            支持 WebDAV、GitHub、Gitee 自动云同步和桌面版 JSON 双向导入导出。提醒预排未来
            30 天、最多 64
            条，每次打开和编辑后刷新；送达时间受系统省电策略影响。
          </p>
          <p class="version">FluentCal Mobile · 与桌面版共享数据格式</p>
        </section>
      </main>
      <nav class="bottom-nav">
        <button
          v-for="item in [
            ['agenda', '日程', 'list'],
            ['calendar', '月历', 'calendar'],
            ['gantt', '甘特', 'chart'],
            ['settings', '设置', 'settings'],
          ] as const"
          :key="item[0]"
          type="button"
          :class="{ active: tab === item[0] }"
          @click="tab = item[0]"
        >
          <FIcon :name="item[2]" :size="21" /><span>{{ item[1] }}</span
          ><em v-if="item[0] === 'agenda' && taskCount">{{
            taskCount > 99 ? "99+" : taskCount
          }}</em>
        </button>
      </nav>
    </div>
    <MobileEditor
      ref="editorRef"
      :group-id="editorGroup"
      :open="editorOpen"
      :task="editorTask"
      :date="editorDate"
      @close="closeEditor"
      @saved="notify"
    />
    <div v-if="pendingImport && mergePlan" class="import-layer">
      <section role="dialog" aria-modal="true" aria-label="合并备份">
        <h2>合并备份</h2>
        <p>
          新增 {{ mergePlan.added }} 项，更新 {{ mergePlan.updated }} 项，保留
          {{ mergePlan.kept }} 项。
        </p>
        <p>
          同一个任务保留修改时间较新的版本；现有其他任务和本机外观设置保留。删除操作不会通过备份合并传播。
        </p>
        <button :disabled="importing" @click="pendingImport = undefined">
          取消</button
        ><button
          class="primary-btn"
          :disabled="importing"
          @click="confirmImport"
        >
          {{ importing ? "导入中" : "确认合并" }}
        </button>
      </section>
    </div>
    <Transition name="toast"
      ><div v-if="toast" class="toast">{{ toast }}</div></Transition
    >
  </div>
</template>

<style scoped>
:global(html),
:global(body),
:global(#app) {
  min-height: 100%;
  background: var(--solid-base);
}
:global(body) {
  overflow: hidden;
}
.mobile-app {
  height: 100dvh;
  display: flex;
  flex-direction: column;
  background: var(--solid-base);
  color: var(--text-primary);
  font-size: 14px;
  padding-bottom: calc(70px + env(safe-area-inset-bottom));
  padding-top: env(safe-area-inset-top);
}
.sync-indicator { color: var(--accent-text); font-size: 10px; }
.sync-indicator.failed { color: var(--critical); }
.app-bar {
  position: sticky;
  top: 0;
  z-index: 10;
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 64px;
  padding: 10px 18px;
  background: color-mix(in srgb, var(--solid-base) 92%, transparent);
  backdrop-filter: blur(18px);
  border-bottom: 1px solid var(--divider);
}
.brand {
  display: flex;
  align-items: center;
  gap: 10px;
}
.brand-mark,
.profile-icon {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  border-radius: 11px;
  background: linear-gradient(
    145deg,
    var(--accent-light1),
    var(--accent-dark2)
  );
  color: #fff;
  font-weight: 800;
  font-size: 20px;
  box-shadow: 0 4px 10px color-mix(in srgb, var(--accent-fill) 28%, transparent);
}
.brand strong {
  display: block;
  font-size: 16px;
  letter-spacing: 0.2px;
}
.brand small {
  display: block;
  margin-top: 1px;
  color: var(--text-secondary);
  font-size: 11px;
}
.bar-actions {
  display: flex;
  gap: 4px;
}
.bar-btn,
.nav-btn,
.today-btn {
  display: grid;
  place-items: center;
  border: 0;
  background: transparent;
  color: var(--text-secondary);
}
.bar-btn {
  width: 38px;
  height: 38px;
  border-radius: 12px;
}
.bar-btn:active,
.nav-btn:active {
  background: var(--subtle-fill-pressed);
}
.search-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 8px 16px 0;
  padding: 9px 12px;
  border-radius: 12px;
  background: var(--control-fill);
}
.search-row input {
  flex: 1;
  min-width: 0;
  border: 0;
  outline: 0;
  background: transparent;
  color: inherit;
}
.search-row button {
  border: 0;
  background: transparent;
  color: var(--text-secondary);
  font-size: 20px;
}
.app-main {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}
.content {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0 16px 18px;
}
.agenda-page,
.calendar-page,
.gantt-page,
.settings-page {
  max-width: 720px;
  margin: 0 auto;
  padding-top: 18px;
}
.hero {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 2px 20px;
}
.eyebrow {
  margin: 0 0 2px;
  color: var(--text-secondary);
  font-size: 13px;
}
h1 {
  margin: 0;
  font-size: 32px;
  letter-spacing: -0.8px;
}
h1 span {
  color: var(--text-secondary);
  font-size: 16px;
  font-weight: 450;
  letter-spacing: 0;
}
.primary-round {
  width: 46px;
  height: 46px;
  border: 0;
  border-radius: 16px;
  background: var(--accent-fill);
  color: var(--text-on-accent);
  font-size: 26px;
  box-shadow: 0 7px 16px color-mix(in srgb, var(--accent-fill) 28%, transparent);
}
.selected-strip {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin: -6px 0 14px;
  padding: 9px 12px;
  border-radius: 10px;
  background: color-mix(in srgb, var(--accent-fill) 10%, transparent);
  color: var(--accent-text);
  font-size: 12px;
}
.selected-strip button {
  border: 0;
  background: transparent;
  color: inherit;
  font-weight: 600;
}
.day-section {
  margin: 12px 0 20px;
}
.day-heading {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin: 0 4px 8px;
}
.day-heading strong {
  display: block;
  font-size: 16px;
}
.day-heading strong.today {
  color: var(--accent-text);
}
.day-heading span {
  display: block;
  margin-top: 1px;
  color: var(--text-secondary);
  font-size: 12px;
}
.day-heading button {
  width: 30px;
  height: 30px;
  border: 0;
  border-radius: 9px;
  background: var(--subtle-fill-hover);
  color: var(--accent-text);
  font-size: 20px;
}
.task-card {
  display: flex;
  align-items: center;
  width: 100%;
  min-height: 68px;
  margin: 7px 0;
  padding: 12px 10px 12px 0;
  border: 1px solid var(--card-stroke);
  border-radius: 15px;
  background: var(--card-fill);
  color: inherit;
  text-align: left;
  box-shadow: var(--shadow-card);
}
.task-accent {
  width: 4px;
  align-self: stretch;
  flex: none;
  margin-right: 11px;
  border-radius: 0 4px 4px 0;
}
.check {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  position: relative;
  flex: none;
  margin-right: 10px;
  border: 1.5px solid var(--control-strong-fill);
  border-radius: 50%;
  color: #fff;
  font-size: 13px;
}
.check::before { content: ""; position:absolute; inset:-7px; }
.check.checked {
  border-color: var(--accent-fill);
  background: var(--accent-fill);
}
.task-main {
  border: 0;
  background: transparent;
  color: inherit;
  text-align: left;
  min-height: 44px;
  display: flex;
  min-width: 0;
  flex: 1;
  flex-direction: column;
  gap: 4px;
}
.task-main strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 14px;
}
.task-meta {
  color: var(--text-secondary);
  font-size: 11px;
}
.mini-progress {
  display: block;
  width: 92px;
  height: 3px;
  overflow: hidden;
  border-radius: 2px;
  background: var(--divider);
}
.mini-progress i {
  display: block;
  height: 100%;
  border-radius: inherit;
}
.task-card.done .task-main strong {
  color: var(--text-secondary);
  text-decoration: line-through;
}
.chevron {
  color: var(--text-tertiary);
  margin-left: 8px;
}
.empty-state {
  display: flex;
  align-items: center;
  flex-direction: column;
  padding: 52px 18px;
  text-align: center;
}
.empty-icon {
  display: grid;
  place-items: center;
  width: 60px;
  height: 60px;
  border-radius: 22px;
  background: color-mix(in srgb, var(--success) 16%, transparent);
  color: var(--success);
  font-size: 28px;
}
.empty-state h2 {
  margin: 17px 0 4px;
  font-size: 19px;
}
.empty-state p {
  margin: 0 0 20px;
  color: var(--text-secondary);
  font-size: 13px;
}
.primary-btn {
  border: 0;
  border-radius: 10px;
  padding: 10px 15px;
  background: var(--accent-fill);
  color: var(--text-on-accent);
  font-weight: 600;
}
.primary-btn.small {
  padding: 8px 11px;
  font-size: 12px;
}
.agenda-more {
  display: flex;
  justify-content: center;
  padding: 5px 0 20px;
}
.agenda-more button {
  display: flex;
  align-items: center;
  gap: 3px;
  border: 0;
  background: transparent;
  color: var(--accent-text);
  font-size: 13px;
}
.calendar-toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 2px 0 15px;
}
.calendar-toolbar h2 {
  flex: 1;
  margin: 0 6px;
  font-size: 20px;
}
.nav-btn {
  width: 36px;
  height: 36px;
  border-radius: 10px;
  font-size: 28px;
}
.today-btn {
  margin-left: auto;
  padding: 7px 10px;
  border-radius: 9px;
  background: var(--subtle-fill-hover);
  color: var(--accent-text);
  font-size: 12px;
}
.weekday-row,
.month-grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 4px;
}
.weekday-row {
  margin-bottom: 5px;
}
.weekday-row span {
  padding: 4px 0;
  color: var(--text-secondary);
  text-align: center;
  font-size: 11px;
}
.weekday-row .rest {
  color: var(--critical);
}
.month-grid {
  padding: 6px;
  border: 1px solid var(--card-stroke);
  border-radius: 16px;
  background: var(--card-fill);
}
.month-day {
  position: relative;
  min-height: 54px;
  padding: 7px 2px 4px;
  border: 1px solid transparent;
  border-radius: 11px;
  background: transparent;
  color: inherit;
}
.month-day b {
  display: block;
  font-size: 14px;
  font-weight: 550;
}
.month-day small {
  display: block;
  height: 14px;
  margin-top: 2px;
  color: var(--text-secondary);
  font-size: 10px;
}
.month-day i {
  display: block;
  width: 5px;
  height: 5px;
  margin: 1px auto 0;
  border-radius: 50%;
}
.month-day.dim {
  opacity: 0.35;
}
.month-day.rest b {
  color: var(--critical);
}
.month-day.today b {
  color: var(--accent-text);
}
.month-day.today {
  background: color-mix(in srgb, var(--accent-fill) 10%, transparent);
}
.month-day.selected {
  border-color: var(--accent-fill);
  background: color-mix(in srgb, var(--accent-fill) 15%, transparent);
}
.selected-day {
  margin-top: 20px;
}
.selected-day-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.selected-day h3 {
  margin: 0;
  font-size: 16px;
}
.muted {
  color: var(--text-secondary);
  font-size: 13px;
}
.compact-task {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  min-height: 46px;
  margin-top: 7px;
  padding: 9px 11px;
  border: 1px solid var(--card-stroke);
  border-radius: 11px;
  background: var(--card-fill);
  color: inherit;
  text-align: left;
}
.compact-task .task-accent {
  flex: none;
  width: 3px;
  height: 25px;
  margin: 0;
}
.compact-task span {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.compact-task small {
  color: var(--text-secondary);
}
.settings-profile {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 4px 3px 20px;
}
.profile-icon {
  width: 46px;
  height: 46px;
  border-radius: 15px;
  font-size: 25px;
}
.settings-profile h2 {
  margin: 0;
  font-size: 20px;
}
.settings-profile p {
  margin: 2px 0 0;
  color: var(--text-secondary);
  font-size: 12px;
}
.settings-card {
  margin: 13px 0;
  overflow: hidden;
  border: 1px solid var(--card-stroke);
  border-radius: 15px;
  background: var(--card-fill);
}
.settings-card h3 {
  margin: 0;
  padding: 13px 14px 7px;
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 550;
}
.setting-row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  min-height: 50px;
  padding: 10px 14px;
  border: 0;
  border-top: 1px solid var(--divider);
  background: transparent;
  color: inherit;
  text-align: left;
}
.setting-row > span {
  flex: 1;
}
.setting-row select {
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--text-secondary);
  text-align: right;
}
.setting-row small {
  color: var(--text-secondary);
  font-size: 11px;
}
.setting-row.action:active {
  background: var(--subtle-fill-pressed);
}
.setting-row.danger {
  color: var(--critical);
}
.hidden {
  display: none;
}
.version {
  margin: 22px 0;
  color: var(--text-tertiary);
  text-align: center;
  font-size: 11px;
}
.bottom-nav {
  position: fixed;
  right: 0;
  bottom: 0;
  left: 0;
  z-index: 20;
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  padding: 7px 10px calc(7px + env(safe-area-inset-bottom));
  border-top: 1px solid var(--divider);
  background: color-mix(in srgb, var(--solid-base) 94%, transparent);
  backdrop-filter: blur(18px);
}
.bottom-nav button {
  position: relative;
  display: flex;
  align-items: center;
  flex-direction: column;
  gap: 3px;
  min-height: 48px;
  border: 0;
  border-radius: 12px;
  background: transparent;
  color: var(--text-secondary);
  font-size: 10px;
}
.bottom-nav button.active {
  background: color-mix(in srgb, var(--accent-fill) 12%, transparent);
  color: var(--accent-text);
  font-weight: 600;
}
.bottom-nav em {
  position: absolute;
  top: 0;
  right: calc(50% - 20px);
  min-width: 15px;
  padding: 1px 3px;
  border-radius: 8px;
  background: var(--critical);
  color: #fff;
  font-size: 9px;
  font-style: normal;
}
.toast {
  position: fixed;
  right: 18px;
  bottom: calc(82px + env(safe-area-inset-bottom));
  left: 18px;
  z-index: 150;
  padding: 11px 14px;
  border-radius: 11px;
  background: var(--flyout-fill);
  box-shadow: var(--shadow-flyout);
  text-align: center;
  font-size: 13px;
}
.toast-enter-active,
.toast-leave-active {
  transition:
    opacity 0.2s,
    transform 0.2s;
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(10px);
}
@media (min-width: 700px) {
  .content {
    padding-left: 24px;
    padding-right: 24px;
  }
  .bottom-nav {
    left: 50%;
    width: 560px;
    transform: translateX(-50%);
    border: 1px solid var(--divider);
    border-bottom: 0;
    border-radius: 17px 17px 0 0;
  }
}
.error-banner,
.sample-notice {
  padding: 10px 14px;
  background: var(--caution-bg);
  color: var(--text-primary);
  font-size: 12px;
  border-radius: 10px;
  margin: 8px 0;
}
.error-banner button,
.sample-notice button {
  border: 0;
  padding: 8px;
  color: var(--accent-text);
  background: transparent;
}
.loading {
  text-align: center;
  margin: 40px 0;
}
.data-note {
  font-size: 12px;
  color: var(--text-secondary);
  line-height: 1.7;
}
.import-layer {
  position: fixed;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 20px;
  z-index: 120;
  background: #0007;
}
.import-layer section {
  max-width: 440px;
  background: var(--solid-base);
  padding: 22px;
  border-radius: 18px;
}
.import-layer p {
  line-height: 1.6;
}
.import-layer button {
  min-height: 44px;
  margin-right: 10px;
  padding: 10px 16px;
  border: 0;
  border-radius: 10px;
}
.month-day small {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.bottom-nav button {
  padding-top: 6px;
}
.bottom-nav button .f-icon {
  min-height: 21px;
}
</style>
