import type { Settings } from './types';

export const DEFAULT_SETTINGS: Settings = {
  general: { autostart: true, seeded: false },
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
    fontScale: 100,
    cellGap: 3,
    lineSpacing: 2,
    todayColor: 'accent',
    restBgColor: 'auto',
    holidayTextColor: 'auto',
    termTextColor: 'auto',
  },
  calendar: {
    view: 'month',
    weekStart: 1,
    monthRows: 'auto',
    weeksRows: 5,
    weeksPast: 1,
    showLunar: true,
    showHolidays: true,
    showWeekNumbers: false,
    showAlmanac: false,
    holidayCountdown: true,
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
  // Keep in sync with cloud/service.rs Config::from_settings (absent fields use these too).
  sync: {
    enabled: false,
    provider: '',
    interval: 5,
    proxy: '',
    webdav: { url: '', user: '', folder: 'FluentCal' },
    github: { repo: '', branch: '', path: '' },
    gitee: { repo: '', branch: '', path: '' },
    gdrive: { clientId: '', email: '' },
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
  return out as T;
}
