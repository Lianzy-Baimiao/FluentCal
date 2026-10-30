/** YYYY-MM-DD in local time. */
export type ISODate = string;

export type RepeatFreq =
  | 'daily'
  | 'weekly'
  | 'monthly'
  | 'yearly'
  | 'workday' // Monday–Friday
  | 'legalWorkday' // working days per the official holiday schedule (incl. 调休)
  | 'lunarYearly'
  | 'monthLastDay'
  | 'ebbinghaus';

export interface RepeatRule {
  freq: RepeatFreq;
  /** Every N days/weeks/months/years. Default 1. */
  interval?: number;
  /** weekly: 0 = Sunday … 6 = Saturday. Defaults to the start date's weekday. */
  weekdays?: number[];
  /** monthly: same date each month, or the same "Nth weekday" (e.g. second Tuesday). */
  monthMode?: 'day' | 'nthWeekday';
  /** Stop after this many occurrences. */
  count?: number;
  /** Last allowed occurrence start (inclusive). */
  until?: ISODate;
}

export interface Task {
  id: string;
  title: string;
  notes?: string;
  start: ISODate;
  /** Inclusive end date. */
  end: ISODate;
  /** HH:mm; empty = all day. */
  time?: string;
  /** TASK_COLORS key or #RRGGBB; empty = default. */
  color?: string;
  groupId?: string;
  /** 0–100 */
  progress: number;
  done: boolean;
  milestone?: boolean;
  /** Predecessor task ids (finish-to-start). */
  deps?: string[];
  /** Minutes before the occurrence (0 = on time). All-day tasks use settings.reminders.allDayTime. */
  reminders?: number[];
  repeat?: RepeatRule;
  /** Repeating tasks: occurrence start dates marked done. */
  doneDates?: ISODate[];
  /** Repeating tasks: occurrence start dates deleted. */
  exDates?: ISODate[];
  /** Show "N days until" at the bottom of the calendar. */
  countdown?: boolean;
  /** Overrides settings.tasks.autoPostpone for this task. */
  autoPostpone?: boolean;
  order: number;
  createdAt: number;
  updatedAt: number;
}

export interface Group {
  id: string;
  name: string;
  color: string;
  collapsed?: boolean;
  order: number;
}

export interface DayMark {
  color: string;
}

export interface HolidayYear {
  year: number;
  fetchedAt: number;
  days: { date: ISODate; name: string; isOffDay: boolean }[];
}

export type ThemeMode = 'system' | 'light' | 'dark';
export type PinMode = 'desktop' | 'top' | 'normal';
export type CalendarView = 'month' | 'weeks' | 'gantt';
export type GanttScale = 'day' | 'week' | 'month';

export interface Settings {
  general: { autostart: boolean; seeded: boolean };
  theme: {
    mode: ThemeMode;
    /** 'system' or #RRGGBB */
    accent: string;
    reduceMotion: boolean;
  };
  desk: {
    pinMode: PinMode;
    locked: boolean;
    clickThrough: boolean;
    /** Desktop mode: clicks don't activate the window (keeps Win+D working). */
    noActivate: boolean;
    acrylic: boolean;
    /** 'auto' (follows theme) or #RRGGBB */
    bgColor: string;
    /** 0–100 */
    bgOpacity: number;
    /** Opacity of the day cells, 0–100 */
    cellOpacity: number;
    radius: number;
    shadow: boolean;
    /** 'auto' | 'on' | 'off' */
    textShadow: 'auto' | 'on' | 'off';
    fontScale: number;
    cellGap: number;
    lineSpacing: number;
    todayColor: string;
    restBgColor: string;
    holidayTextColor: string;
    termTextColor: string;
    bounds?: { x: number; y: number; w: number; h: number };
  };
  calendar: {
    view: CalendarView;
    weekStart: 0 | 1;
    monthRows: 'auto' | 'six';
    weeksRows: number;
    weeksPast: number;
    showLunar: boolean;
    showHolidays: boolean;
    showWeekNumbers: boolean;
    showAlmanac: boolean;
    /** Countdown to the next statutory holiday in the countdown bar. */
    holidayCountdown: boolean;
    /** 0 = Sunday … 6 = Saturday */
    restDays: number[];
  };
  tasks: {
    showCompleted: boolean;
    completedLast: boolean;
    autoPostpone: boolean;
    defaultColor: string;
  };
  gantt: {
    scale: GanttScale;
    tableWidth: number;
    showTable: boolean;
    hideDone: boolean;
    hideSingleDay: boolean;
    showDeps: boolean;
    groupFilter: string[];
  };
  reminders: {
    enabled: boolean;
    /** Windows notification, popup card in the corner, or both. */
    style: 'toast' | 'popup' | 'both';
    allDayTime: string;
    defaultOffsets: number[];
  };
  holidays: { autoUpdate: boolean; lastCheck: number };
  shortcuts: { toggle: string; newTask: string };
  /** Cloud sync (SYN). Per PC; passwords and tokens are in Windows Credential Manager. */
  sync: {
    enabled: boolean;
    /** '' = off; 'folder' = a shared data folder (DAT-04), not a network sync. */
    provider: SyncProvider;
    /** Minutes between pulls; 0 = only after start and edits. */
    interval: number;
    /** '' = Windows system proxy, 'direct' = none, otherwise http://host:port. */
    proxy: string;
    webdav: { url: string; user: string; folder: string };
    github: { repo: string; branch: string; path: string };
    gitee: { repo: string; branch: string; path: string };
    gdrive: { clientId: string; email: string };
  };
}

export type SyncProvider = '' | 'webdav' | 'github' | 'gitee' | 'gdrive' | 'folder';

export interface Doc {
  version: number;
  tasks: Record<string, Task>;
  groups: Record<string, Group>;
  dayMarks: Record<ISODate, DayMark>;
  holidays: Record<string, HolidayYear>;
  settings: Settings;
}

export type Collection = 'tasks' | 'groups' | 'dayMarks' | 'holidays';
