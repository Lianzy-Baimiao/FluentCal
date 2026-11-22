import type { Settings } from './types';
import { parseRgba, rgbaHex } from './lib/rgba';

export const DEFAULT_SETTINGS: Settings = {
  general: { language: 'system', autostart: true, seeded: false, onboarded: false },
  theme: { mode: 'system', accent: 'system', reduceMotion: false },
  desk: {
    pinMode: 'desktop',
    locked: true,
    clickThrough: false,
    noActivate: true,
    acrylic: false,
    bgColor: 'auto',
    bgOpacity: 72,
    cellOpacity: 55,
    radius: 8,
    shadow: true,
    textShadow: 'auto',
    fontFamily: 'auto',
    textColor: 'auto',
    taskTextMode: 'calendar',
    textOutlineWidth: 1,
    fontScale: 100,
    cellGap: 3,
    lineSpacing: 2,
    todayBorderColor: '#006DC0',
    todayBorderWidth: 2,
    todayFillColor: '#0078D426',
    restBgColor: 'auto',
    holidayTextColor: 'auto',
    termTextColor: 'auto',
  },
  calendar: {
    view: 'month',
    listRange: 'upcoming',
    listGroupLabels: false,
    weekStart: 1,
    monthRows: 'auto',
    fitRows: false,
    sidebarMode: 'inline',
    sidebarClose: 'button',
    barDays: true,
    barProgress: true,
    barDone: true,
    weeksRows: 5,
    weeksPast: 1,
    showLunar: true,
    showHolidays: true,
    showWeekNumbers: false,
    showAlmanac: false,
    holidayCountdown: false,
    countdownFestival: true,
    countdownTerm: false,
    ungroupedColor: '',
    countdownMarked: true,
    countdownGroups: [],
    restDays: [0, 6],
  },
  tasks: { showCompleted: true, completedLast: true, autoPostpone: false, defaultColor: '' },
  gantt: {
    scale: 'day',
    tableWidth: 220,
    showTable: true,
    hideDone: false,
    hideSingleDay: true,
    showDeps: true,
    groupFilter: [],
  },
  reminders: { enabled: true, style: 'both', allDayTime: '09:00', defaultOffsets: [] },
  holidays: { autoUpdate: true, lastCheck: 0 },
  shortcuts: { toggle: 'Ctrl+Alt+D', newTask: 'Ctrl+Alt+N' },
  web: { enabled: false, port: 18765 },
  mcp: { enabled: false, port: 18766, write: true },
  // Keep in sync with cloud/service.rs Config::from_settings (absent fields use these too).
  sync: {
    enabled: false,
    provider: '',
    interval: 5,
    proxy: '',
    webdav: { url: '', user: '', folder: 'FluentCal' },
    github: { repo: '', branch: '', path: '' },
    gitee: { repo: '', branch: '', path: '' },
  },
};

type Json = unknown;

function isPlainObject(v: Json): v is Record<string, Json> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/**
 * Deep-merges stored values over defaults. A stored value whose type differs from the
 * default (corrupt or from an older version) is ignored, so the UI always sees a valid shape.
 * Keys absent from the defaults are kept (e.g. desk.bounds).
 */
export function mergeDefaults<T>(defaults: T, stored: Json): T {
  if (!isPlainObject(defaults) || !isPlainObject(stored)) {
    if (stored === undefined || stored === null) return structuredClone(defaults);
    if (Array.isArray(defaults)) return (Array.isArray(stored) ? stored : structuredClone(defaults)) as T;
    return (typeof stored === typeof defaults ? stored : defaults) as T;
  }
  const out: Record<string, Json> = {};
  for (const [k, dv] of Object.entries(defaults)) out[k] = mergeDefaults(dv, stored[k]);
  for (const [k, sv] of Object.entries(stored)) if (!(k in out)) out[k] = sv;
  // Migrate the former exclusive today fill/outline choice on read, without writing it
  // back or replacing explicitly configured values from a newer version.
  const savedDesk = stored.desk;
  const mergedDesk = out.desk;
  if (isPlainObject(savedDesk) && isPlainObject(mergedDesk) && ['todayColor', 'todayStyle', 'todayOpacity'].some(k => k in savedDesk)) {
    const legacyColor = typeof savedDesk.todayColor === 'string' ? savedDesk.todayColor : 'accent';
    const accent = isPlainObject(stored.theme) && typeof stored.theme.accent === 'string' && stored.theme.accent !== 'system' ? stored.theme.accent : '#0078D4';
    const fill = savedDesk.todayStyle === 'fill';
    const opacity = typeof savedDesk.todayOpacity === 'number' ? Math.max(0, Math.min(100, savedDesk.todayOpacity)) / 100 : 0.35;
    const parsed = parseRgba(legacyColor === 'accent' ? accent : legacyColor) ?? [0, 120, 212, 1];
    const color = rgbaHex([parsed[0], parsed[1], parsed[2], parsed[3] * opacity]);
    if (!('todayBorderColor' in savedDesk)) mergedDesk.todayBorderColor = fill ? '' : color;
    if (!('todayBorderWidth' in savedDesk)) mergedDesk.todayBorderWidth = fill ? 0 : 1;
    if (!('todayFillColor' in savedDesk)) mergedDesk.todayFillColor = fill ? color : '';
  }
  return out as T;
}
