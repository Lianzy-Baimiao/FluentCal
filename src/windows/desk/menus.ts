import { showMenu, TASK_COLORS, type MenuItem } from '@/fluent';
import type { ISODate, PinMode, Task, ThemeMode } from '@/types';
import { call, screenTitle, type Screen } from '@/api';
import { syncNow, syncStatus } from '@/composables/sync';
import { doc, patchSettings, storeState } from '@/stores/doc';
import { addDays } from '@/lib/date';
import type { Occurrence } from '@/lib/recur';
import { inGantt } from '@/lib/gantt';
import {
  type DeleteMode,
  deleteOccurrence,
  deleteTask,
  duplicateTask,
  moveTask,
  saveTask,
  setColor,
  setDayMark,
  setOccurrenceDone,
} from '@/lib/tasks';
import { goToday, notify, openEditor, openInBrowser, openSettings, ui } from './state';

type At = Parameters<typeof showMenu>[1];

export const colorSwatches = (selected: string | undefined, withDefault = true) => [
  ...(withDefault ? [{ key: '', color: 'transparent', title: '默认', selected: !selected }] : []),
  ...TASK_COLORS.map((c) => ({ key: c.key, color: c.bar, title: c.label, selected: selected === c.key })),
];

/** Deletes a task (or one occurrence) and offers undo. */
export async function removeTask(t: Task, date?: ISODate, mode: DeleteMode = 'all'): Promise<void> {
  if (storeState.readOnly) return;
  if (t.repeat && date) await deleteOccurrence(t, date, mode);
  else await deleteTask(t.id);
  notify(`已删除“${t.title.length > 16 ? `${t.title.slice(0, 16)}…` : t.title}”`, true);
}

export function chipMenu(occ: Occurrence, at: At): Promise<void> {
  if (storeState.readOnly) return Promise.resolve(); // web view: every item edits
  const t = occ.task;
  const items: (MenuItem | false)[] = [
    {
      label: occ.done ? '标记为未完成' : '标记为完成',
      icon: 'completed',
      shortcut: 'Space',
      action: () => void setOccurrenceDone(t, occ.date, !occ.done),
    },
    { swatches: colorSwatches(t.color), onSwatch: (k) => void setColor(t, k) },
    { separator: true },
    { label: '编辑…', icon: 'edit', shortcut: 'Enter', action: () => openEditor({ mode: 'edit', taskId: t.id, date: occ.date }) },
    { label: '复制一份', icon: 'copy', action: () => void duplicateTask(t) },
    !t.repeat && { label: '推迟一天', icon: 'forward', action: () => void moveTask(t, addDays(t.start, 1)) },
    !t.repeat &&
      t.start === t.end && {
        label: '未完成时自动顺延',
        checked: t.autoPostpone ?? doc.settings.tasks.autoPostpone,
        action: () => void saveTask({ ...t, autoPostpone: !(t.autoPostpone ?? doc.settings.tasks.autoPostpone) }),
      },
    { label: '倒数日', icon: 'flag', checked: !!t.countdown, action: () => void saveTask({ ...t, countdown: !t.countdown || undefined }) },
    doc.settings.calendar.view !== 'gantt' &&
      inGantt(t, { ...doc.settings.gantt }) && {
        label: '在甘特图中查看',
        icon: 'gantt',
        action: () => {
          ui.ganttFocus = t.id;
          patchSettings({ calendar: { view: 'gantt' } });
        },
      },
    { separator: true },
    t.repeat
      ? {
          label: '删除',
          icon: 'delete',
          danger: true,
          children: [
            { label: '仅删除这一次', action: () => void removeTask(t, occ.date, 'this') },
            { label: '删除这一次及以后', action: () => void removeTask(t, occ.date, 'future') },
            { label: '删除全部重复', danger: true, action: () => void removeTask(t, occ.date, 'all') },
          ],
        }
      : { label: '删除', icon: 'delete', danger: true, shortcut: 'Del', action: () => void removeTask(t) },
  ];
  return showMenu(items.filter(Boolean) as MenuItem[], at);
}

export function cellMenu(date: ISODate, at: At): Promise<void> {
  if (storeState.readOnly) return Promise.resolve();
  const mark = doc.dayMarks[date];
  const items: (MenuItem | false)[] = [
    { label: '快速添加', icon: 'add', shortcut: '双击', action: () => (ui.quickAdd = { date }) },
    { label: '新建任务…', icon: 'edit', action: () => openEditor({ mode: 'new', start: date }) },
    { separator: true },
    { label: '日期颜色', icon: 'color', disabled: true },
    { swatches: colorSwatches(mark?.color, false), onSwatch: (k) => void setDayMark(date, k) },
    !!mark && { label: '清除日期颜色', icon: 'close', action: () => void setDayMark(date, null) },
    { separator: true },
    { label: '回到今天', icon: 'today', shortcut: 'Home', action: goToday },
    { label: '在浏览器中查看', icon: 'globe', action: () => void openInBrowser() },
  ];
  return showMenu(items.filter(Boolean) as MenuItem[], at);
}

const THEMES: { v: ThemeMode; label: string }[] = [
  { v: 'system', label: '跟随系统' },
  { v: 'light', label: '浅色' },
  { v: 'dark', label: '深色' },
];
const PINS: { v: PinMode; label: string }[] = [
  { v: 'desktop', label: '嵌入桌面' },
  { v: 'top', label: '始终置顶' },
  { v: 'normal', label: '普通窗口' },
];

export async function moreMenu(at: At): Promise<void> {
  const s = doc.settings;
  const screens = (await call<Screen[]>('screens_list').catch(() => null)) ?? [];
  const items: MenuItem[] = [
    { label: '新建任务…', icon: 'add', shortcut: 'Ctrl+N', action: () => openEditor({ mode: 'new' }) },
    { label: '搜索', icon: 'search', shortcut: 'Ctrl+F', action: () => (ui.searchOpen = true) },
    ...(syncStatus.enabled ? [{ label: '立即同步', icon: 'sync', action: () => void syncNow() }] : []),
    { label: '在浏览器中查看', icon: 'globe', action: () => void openInBrowser() },
    { separator: true },
    {
      label: '显示已完成任务',
      checked: s.tasks.showCompleted,
      action: () => patchSettings({ tasks: { showCompleted: !s.tasks.showCompleted } }),
    },
    {
      label: '主题',
      icon: 'brightness',
      children: THEMES.map((x) => ({
        label: x.label,
        radio: true,
        checked: s.theme.mode === x.v,
        action: () => patchSettings({ theme: { mode: x.v } }),
      })),
    },
    {
      label: '窗口层级',
      icon: 'window',
      children: PINS.map((x) => ({
        label: x.label,
        radio: true,
        checked: s.desk.pinMode === x.v,
        action: () => patchSettings({ desk: { pinMode: x.v } }),
      })),
    },
    ...(screens.length > 1
      ? [
          {
            label: '显示在',
            icon: 'desktop',
            children: screens.map((x) => ({
              label: screenTitle(x),
              radio: true,
              checked: x.current,
              action: () => void call('desk_move_to_screen', { index: x.index }),
            })),
          },
        ]
      : []),
    {
      label: s.desk.locked ? '调整位置和大小' : '完成调整',
      icon: s.desk.locked ? 'unlock' : 'lock',
      action: () => patchSettings({ desk: { locked: !s.desk.locked } }),
    },
    { label: '外观…', icon: 'palette', action: () => openSettings('appearance') },
    { label: '设置…', icon: 'settings', action: () => openSettings() },
    { separator: true },
    { label: '隐藏日历', icon: 'eyeOff', action: () => void call('desk_hide') },
    { label: '退出', icon: 'power', action: () => void call('app_quit') },
  ];
  return showMenu(items, at, { placement: 'bottom-end' });
}
