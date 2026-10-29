import type { Doc, Group, Task } from '@/types';
import { addDays, daysInMonth, diffDays, maxDate, parts, today } from './date';
import { describeRule, occurrences } from './recur';

// ---------------------------------------------------------------------------------------
// CSV (opens in Excel; caller writes it with a UTF-8 BOM)

function csvCell(v: string): string {
  // Neutralise spreadsheet formulas (CSV injection).
  const safe = /^[=+\-@\t\r]/.test(v) ? `'${v}` : v;
  return /[",\r\n]/.test(safe) ? `"${safe.replace(/"/g, '""')}"` : safe;
}

export function toCSV(tasks: Task[], groups: Record<string, Group>): string {
  const head = ['标题', '开始日期', '结束日期', '时间', '分组', '进度', '状态', '重复', '备注'];
  const rows = [...tasks]
    .sort((a, b) => (a.start < b.start ? -1 : a.start > b.start ? 1 : a.order - b.order))
    .map((t) => [
      t.title,
      t.start,
      t.end,
      t.time ?? '',
      (t.groupId && groups[t.groupId]?.name) || '',
      `${t.done ? 100 : t.progress}%`,
      t.done ? '已完成' : '未完成',
      t.repeat ? describeRule(t.repeat, t.start) : '',
      t.notes ?? '',
    ]);
  return [head, ...rows].map((r) => r.map(csvCell).join(',')).join('\r\n');
}

// ---------------------------------------------------------------------------------------
// iCalendar (RFC 5545)

const compact = (iso: string) => iso.replace(/-/g, '');
const ICS_DAYS = ['SU', 'MO', 'TU', 'WE', 'TH', 'FR', 'SA'];

function esc(s: string): string {
  return s.replace(/\\/g, '\\\\').replace(/;/g, '\\;').replace(/,/g, '\\,').replace(/\r?\n/g, '\\n');
}

/** Folds lines longer than 75 octets (UTF-8) as required by RFC 5545. */
function fold(line: string): string {
  const enc = new TextEncoder();
  if (enc.encode(line).length <= 75) return line;
  const out: string[] = [];
  let cur = '';
  let bytes = 0;
  for (const ch of line) {
    const b = enc.encode(ch).length;
    if (bytes + b > (out.length ? 74 : 75)) {
      out.push(cur);
      cur = '';
      bytes = 0;
    }
    cur += ch;
    bytes += b;
  }
  out.push(cur);
  return out.join('\r\n ');
}

function stamp(d: Date): string {
  return d.toISOString().replace(/[-:]/g, '').replace(/\.\d{3}/, '');
}

function rrule(t: Task): string | null {
  // Rules are written to give exactly the dates recur.ts gives (same week start, same
  // clamping of the 29th–31st), and DTSTART is the first real occurrence: RFC 5545 always
  // counts DTSTART as an occurrence even when it doesn't match the rule.
  const r = t.repeat!;
  const i = Math.max(1, r.interval ?? 1);
  const [y, m, day] = parts(t.start);
  const wdOf = (iso: string) => new Date(`${iso}T00:00`).getDay();
  let s: string;
  switch (r.freq) {
    case 'daily':
      s = `FREQ=DAILY;INTERVAL=${i}`;
      break;
    case 'weekly': {
      const wd = r.weekdays?.length ? r.weekdays : [wdOf(t.start)];
      // recur.ts counts "every N weeks" from the Sunday of the start week.
      s = `FREQ=WEEKLY;INTERVAL=${i};WKST=SU;BYDAY=${wd.map((d) => ICS_DAYS[d]).join(',')}`;
      break;
    }
    case 'monthly':
      if (r.monthMode === 'nthWeekday') {
        const nth = Math.ceil(day / 7);
        const last = nth >= 5 || day + 7 > daysInMonth(y, m);
        s = `FREQ=MONTHLY;INTERVAL=${i};BYDAY=${last ? -1 : nth}${ICS_DAYS[wdOf(t.start)]}`;
      } else if (day > 28) {
        // The 29th–31st fall back to the month's last day, like recur.ts.
        s = `FREQ=MONTHLY;INTERVAL=${i};BYMONTHDAY=${Array.from({ length: day - 27 }, (_, k) => 28 + k).join(',')};BYSETPOS=-1`;
      } else s = `FREQ=MONTHLY;INTERVAL=${i};BYMONTHDAY=${day}`;
      break;
    case 'yearly':
      s = m === 2 && day === 29 ? `FREQ=YEARLY;INTERVAL=${i};BYMONTH=2;BYMONTHDAY=28,29;BYSETPOS=-1` : `FREQ=YEARLY;INTERVAL=${i}`;
      break;
    case 'workday':
      s = 'FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR';
      break;
    case 'monthLastDay':
      s = `FREQ=MONTHLY;INTERVAL=${i};BYMONTHDAY=-1`;
      break;
    default:
      return null; // lunar / legal workdays / Ebbinghaus: exported as expanded events
  }
  if (r.count) s += `;COUNT=${r.count}`;
  // UNTIL must have the same value type as DTSTART (floating date-time for timed events).
  else if (r.until) s += `;UNTIL=${compact(r.until)}${t.time ? 'T235959' : ''}`;
  return s;
}

/** First occurrence of a repeating task, ignoring deleted dates (they become EXDATEs). */
function firstOccurrence(t: Task): string {
  return occurrences({ ...t, exDates: undefined }, t.start, addDays(t.start, 800), 1)[0]?.start ?? t.start;
}

function eventLines(t: Task, uidSuffix: string, start: string, end: string, groupName?: string, rule?: string | null): string[] {
  const lines = ['BEGIN:VEVENT', `UID:${t.id}${uidSuffix}@fluentcal`, `DTSTAMP:${stamp(new Date())}`];
  if (t.time) {
    const hm = t.time.replace(':', '');
    lines.push(`DTSTART:${compact(start)}T${hm}00`);
    if (start === end) lines.push('DURATION:PT1H');
    else lines.push(`DTEND:${compact(end)}T${hm}00`);
  } else {
    lines.push(`DTSTART;VALUE=DATE:${compact(start)}`, `DTEND;VALUE=DATE:${compact(addDays(end, 1))}`);
  }
  lines.push(`SUMMARY:${esc(t.title)}`);
  if (t.notes) lines.push(`DESCRIPTION:${esc(t.notes)}`);
  if (groupName) lines.push(`CATEGORIES:${esc(groupName)}`);
  if (rule) {
    lines.push(`RRULE:${rule}`);
    if (t.exDates?.length) {
      const ex = t.exDates.map((d) => (t.time ? `${compact(d)}T${t.time.replace(':', '')}00` : compact(d)));
      lines.push(t.time ? `EXDATE:${ex.join(',')}` : `EXDATE;VALUE=DATE:${ex.join(',')}`);
    }
  }
  lines.push('END:VEVENT');
  return lines;
}

export function toICS(tasks: Task[], groups: Record<string, Group>, now: string = today()): string {
  const lines = ['BEGIN:VCALENDAR', 'VERSION:2.0', 'PRODID:-//FluentCal//Desktop Calendar//ZH', 'CALSCALE:GREGORIAN', 'X-WR-CALNAME:FluentCal'];
  for (const t of tasks) {
    const g = t.groupId ? groups[t.groupId]?.name : undefined;
    if (!t.repeat) {
      lines.push(...eventLines(t, '', t.start, t.end, g));
      continue;
    }
    const rule = rrule(t);
    if (rule) {
      const first = firstOccurrence(t);
      lines.push(...eventLines(t, '', first, addDays(first, diffDays(t.start, t.end)), g, rule));
    } else {
      // Not expressible as RRULE: one event per occurrence, from a year ago to two years ahead
      // (a lunar birthday that started in 1990 still exports this year's date).
      const from = maxDate(t.start, addDays(now, -366));
      for (const o of occurrences(t, from, addDays(now, 732), 800)) {
        lines.push(...eventLines(t, `-${compact(o.date)}`, o.start, o.end, g));
      }
    }
  }
  lines.push('END:VCALENDAR');
  return lines.map(fold).join('\r\n') + '\r\n';
}

// ---------------------------------------------------------------------------------------
// JSON backup import

export interface MergePlan {
  tasks: Record<string, Task>;
  groups: Doc['groups'];
  dayMarks: Doc['dayMarks'];
  added: number;
  updated: number;
  kept: number;
}

/**
 * What a "merge" import writes: new items are added; a task that exists on both sides takes
 * whichever version was edited last (updatedAt); existing groups and day colors are kept.
 */
export function planMerge(current: Pick<Doc, 'tasks' | 'groups' | 'dayMarks'>, incoming: Doc): MergePlan {
  const p: MergePlan = { tasks: {}, groups: {}, dayMarks: {}, added: 0, updated: 0, kept: 0 };
  for (const [id, t] of Object.entries(incoming.tasks)) {
    const cur = current.tasks[id];
    if (!cur) {
      p.tasks[id] = t;
      p.added++;
    } else if ((t.updatedAt ?? 0) > (cur.updatedAt ?? 0)) {
      p.tasks[id] = t;
      p.updated++;
    } else p.kept++;
  }
  for (const [id, g] of Object.entries(incoming.groups)) if (!current.groups[id]) p.groups[id] = g;
  for (const [d, m] of Object.entries(incoming.dayMarks)) if (!current.dayMarks[d]) p.dayMarks[d] = m;
  return p;
}

/** Validates a backup file; returns the document or an error message. */
export function parseBackup(text: string): Doc | string {
  let v: unknown;
  try {
    v = JSON.parse(text);
  } catch {
    return '文件不是有效的 JSON';
  }
  const d = v as Partial<Doc>;
  if (!d || typeof d !== 'object' || typeof d.tasks !== 'object' || d.tasks === null) return '文件中没有任务数据，可能不是 FluentCal 备份';
  for (const [id, t] of Object.entries(d.tasks)) {
    if (!t || typeof t.title !== 'string' || typeof t.start !== 'string' || typeof t.end !== 'string') return `任务 ${id} 的格式不正确`;
  }
  return {
    version: 1,
    tasks: d.tasks,
    groups: d.groups ?? {},
    dayMarks: d.dayMarks ?? {},
    holidays: d.holidays ?? {},
    settings: (d.settings ?? {}) as Doc['settings'],
  };
}
