import { reactive } from 'vue';
import type { ISODate } from '@/types';
import { addDays, addMonths, startOfMonth, startOfWeek } from '@/lib/date';
import { call } from '@/api';
import { doc, patchSettings } from '@/stores/doc';
import { now } from '@/composables/now';

/** UI state shared by the desk window's components. */
export const ui = reactive({
  /** Month view: any day of the shown month. Weeks view: first shown week. */
  cursor: startOfMonth(now.value) as ISODate,
  weeksStart: '' as ISODate,
  /** Keyboard-focused day. */
  selected: now.value as ISODate,
  /** Day highlighted briefly after a search jump. */
  flash: '' as ISODate,
  quickAdd: null as null | { date: ISODate },
  searchOpen: false,
  /** Task the Gantt view should scroll to and select. */
  ganttFocus: '',
  /** Transient message at the bottom of the calendar, e.g. after a delete. */
  toast: null as null | { text: string; undo: boolean },
});

let toastTimer: ReturnType<typeof setTimeout> | undefined;
export function notify(text: string, undo = false): void {
  ui.toast = { text, undo };
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (ui.toast = null), undo ? 6000 : 2500);
}

export type EditorPayload =
  | { mode: 'edit'; taskId: string; date?: ISODate }
  | { mode: 'new'; start?: ISODate; end?: ISODate; groupId?: string; title?: string; milestone?: boolean };

export function openEditor(payload: EditorPayload): void {
  void call('editor_open', { payload });
}

export function openSettings(page?: string): void {
  void call('settings_open', { page });
}

/** First week of the rolling multi-week view (for today, or for `day`). */
export function defaultWeeksStart(day: ISODate = now.value): ISODate {
  const c = doc.settings.calendar;
  return addDays(startOfWeek(day, c.weekStart), -7 * c.weeksPast);
}

export function goToday(): void {
  ui.cursor = startOfMonth(now.value);
  ui.weeksStart = defaultWeeksStart();
  ui.selected = now.value;
  flash(now.value);
}

export function page(delta: number): void {
  if (doc.settings.calendar.view === 'weeks') {
    ui.weeksStart = addDays(ui.weeksStart || defaultWeeksStart(), delta * 7);
  } else {
    ui.cursor = addMonths(startOfMonth(ui.cursor), delta);
  }
}

/** Shows `date` in the calendar (switching away from Gantt if needed) and highlights it. */
export function jumpTo(date: ISODate): void {
  if (doc.settings.calendar.view === 'gantt') patchSettings({ calendar: { view: 'month' } });
  ui.cursor = startOfMonth(date);
  ui.weeksStart = addDays(startOfWeek(date, doc.settings.calendar.weekStart), -7 * doc.settings.calendar.weeksPast);
  ui.selected = date;
  flash(date);
}

let flashTimer: ReturnType<typeof setTimeout> | undefined;
export function flash(date: ISODate): void {
  ui.flash = date;
  clearTimeout(flashTimer);
  flashTimer = setTimeout(() => (ui.flash = ''), 1600);
}
