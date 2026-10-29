import { describe, expect, it } from 'vitest';
import { DEFAULT_SETTINGS, mergeDefaults } from '@/defaults';
import { parseBackup, planMerge, toCSV, toICS } from './export';
import { computeReminders } from './reminders';
import { searchTasks } from './search';
import { sampleData } from './seed';
import { mk } from './testing';

describe('settings defaults', () => {
  it('fills missing keys, ignores wrong types and keeps extras', () => {
    const s = mergeDefaults(DEFAULT_SETTINGS, {
      theme: { mode: 'dark', accent: 42 },
      desk: { bounds: { x: 1, y: 2, w: 3, h: 4 } },
      calendar: { restDays: [0] },
    });
    expect(s.theme.mode).toBe('dark');
    expect(s.theme.accent).toBe('system');
    expect(s.desk.bounds).toEqual({ x: 1, y: 2, w: 3, h: 4 });
    expect(s.desk.bgOpacity).toBe(DEFAULT_SETTINGS.desk.bgOpacity);
    expect(s.calendar.restDays).toEqual([0]);
    expect(mergeDefaults(DEFAULT_SETTINGS, undefined)).toEqual(DEFAULT_SETTINGS);
  });
});

describe('export', () => {
  const t1 = mk({ title: '周报, "重要"', start: '2026-10-30', notes: '第一行\n第二行' });
  const t2 = mk({ title: '=SUM(A1)', start: '2026-10-01', end: '2026-10-03' });

  it('writes safe CSV', () => {
    const csv = toCSV([t1, t2], {});
    const lines = csv.split('\r\n');
    expect(lines[0]).toBe('标题,开始日期,结束日期,时间,分组,进度,状态,重复,备注');
    expect(lines[1].startsWith("'=SUM(A1),2026-10-01,2026-10-03")).toBe(true);
    expect(csv).toContain('"周报, ""重要"""');
    expect(csv).toContain('"第一行\n第二行"');
  });

  it('writes valid iCalendar', () => {
    const weekly = mk({ title: '例会', start: '2026-10-28', time: '10:00', repeat: { freq: 'weekly', weekdays: [3] } });
    const lunar = mk({ title: '生日'.repeat(30), start: '2026-02-17', repeat: { freq: 'lunarYearly', count: 2 } });
    const ics = toICS([t2, weekly, lunar], {}, '2026-10-29');
    expect(ics).toContain('DTSTART;VALUE=DATE:20261001');
    expect(ics).toContain('DTEND;VALUE=DATE:20261004');
    expect(ics).toContain('RRULE:FREQ=WEEKLY;INTERVAL=1;WKST=SU;BYDAY=WE');
    expect(ics).toContain('DTSTART:20261028T100000');
    expect(ics.match(/BEGIN:VEVENT/g)).toHaveLength(4); // lunar rule expanded into 2 events
    const enc = new TextEncoder();
    for (const line of ics.split('\r\n')) expect(enc.encode(line).length).toBeLessThanOrEqual(75);
  });

  it('merges a backup without reverting newer edits', () => {
    const cur = { tasks: { a: mk({ id: 'a', title: '本周改过', start: '2026-10-01', updatedAt: 200 }), b: mk({ id: 'b', title: '旧', start: '2026-10-01', updatedAt: 10 }) }, groups: { g: { id: 'g', name: '现有', color: 'blue', order: 1 } }, dayMarks: {} };
    const inc = parseBackup(JSON.stringify({
      tasks: { a: mk({ id: 'a', title: '周一的备份', start: '2026-10-01', updatedAt: 100 }), b: mk({ id: 'b', title: '备份里更新', start: '2026-10-01', updatedAt: 50 }), c: mk({ id: 'c', title: '新的', start: '2026-10-02' }) },
      groups: { g: { id: 'g', name: '备份名', color: 'red', order: 1 }, h: { id: 'h', name: '新组', color: 'red', order: 2 } },
    }));
    if (typeof inc === 'string') throw new Error(inc);
    const p = planMerge(cur, inc);
    expect(Object.keys(p.tasks).sort()).toEqual(['b', 'c']);
    expect([p.added, p.updated, p.kept]).toEqual([1, 1, 1]);
    expect(Object.keys(p.groups)).toEqual(['h']);
  });

  it('validates backups', () => {
    expect(typeof parseBackup('nope')).toBe('string');
    expect(typeof parseBackup('{"tasks": {"x": {"title": 1}}}')).toBe('string');
    const d = parseBackup(JSON.stringify({ tasks: { a: mk({ id: 'a', start: '2026-10-01' }) } }));
    expect(typeof d).toBe('object');
  });
});

describe('reminders', () => {
  const at = (h: number, m = 0) => new Date(2026, 9, 29, h, m).getTime();
  const timed = mk({ id: 'm', title: '开会', start: '2026-10-29', time: '10:00', reminders: [15] });

  it('fires before a timed task within the window', () => {
    expect(computeReminders([timed], at(9), '09:00').map((r) => r.atMs)).toEqual([at(9, 45)]);
    expect(computeReminders([timed], at(9, 50), '09:00')).toHaveLength(1); // still within grace
    expect(computeReminders([timed], at(10, 0), '09:00')).toHaveLength(0);
  });

  it('uses the all-day time and skips done occurrences', () => {
    const allDay = mk({ start: '2026-10-30', reminders: [0, 1440] });
    expect(computeReminders([allDay], at(8), '09:00').map((r) => new Date(r.atMs).getHours())).toEqual([9, 9]);
    const daily = mk({ start: '2026-10-29', time: '20:00', reminders: [0], repeat: { freq: 'daily' }, doneDates: ['2026-10-29'] });
    expect(computeReminders([daily], at(12), '09:00').map((r) => r.date)).toEqual(['2026-10-30']);
  });
});

describe('search and sample data', () => {
  it('finds tasks by words, status and range', () => {
    const tasks = [mk({ title: '交周报', start: '2026-10-30' }), mk({ title: '周会纪要', start: '2025-01-01', done: true })];
    const opts = { q: '周', range: 'all' as const, status: 'all' as const, color: '' };
    expect(searchTasks(tasks, {}, opts, '2026-10-29').map((h) => h.task.title)).toEqual(['交周报', '周会纪要']);
    expect(searchTasks(tasks, {}, { ...opts, range: '1m' }, '2026-10-29')).toHaveLength(1);
    expect(searchTasks(tasks, {}, { ...opts, status: 'done' }, '2026-10-29')[0].task.title).toBe('周会纪要');
    // A weekly task started in January is found near today and jumps to its nearest occurrence.
    const weekly = mk({ title: '周例会', start: '2026-01-05', repeat: { freq: 'weekly' } });
    const hit = searchTasks([weekly], {}, { ...opts, q: '例会', range: '1m' }, '2026-10-29');
    expect(hit.map((h) => h.date)).toEqual(['2026-10-26']);
  });

  it('creates linked sample tasks', () => {
    const { groups, tasks } = sampleData('2026-10-29');
    expect(groups).toHaveLength(1);
    const ids = new Set(tasks.map((t) => t.id));
    for (const t of tasks) for (const d of t.deps ?? []) expect(ids.has(d)).toBe(true);
    expect(tasks.some((t) => t.milestone)).toBe(true);
  });
});

describe('countdown', () => {
  it('counts down to the next statutory holiday', async () => {
    const { nextHoliday } = await import('./countdown');
    expect(nextHoliday('2026-09-20')).toEqual({ name: '中秋节', date: '2026-09-25' });
    expect(nextHoliday('2026-09-28')).toEqual({ name: '国庆节', date: '2026-10-01' });
    // during the holiday it moves on to the next one
    expect((nextHoliday('2026-10-03')?.date ?? '') > '2026-10-08').toBe(true);
  });

  it('falls back to festival dates when the schedule is not published', async () => {
    const { nextHoliday } = await import('./countdown');
    // 2027 schedule unpublished as of tyme4ts 1.5.4: 元旦 2027-01-01 is the next major festival
    expect(nextHoliday('2026-10-29')).toEqual({ name: '元旦', date: '2027-01-01' });
  });
});

describe('countdown tasks', () => {
  it('keeps a running multi-day occurrence as "today"', async () => {
    const { countdowns } = await import('./countdown');
    const trip = mk({ title: '出差', start: '2026-10-27', end: '2026-10-31', countdown: true, repeat: { freq: 'yearly' } });
    const once = mk({ title: '展会', start: '2026-10-28', end: '2026-10-30', countdown: true });
    const c = countdowns([trip, once], '2026-10-29', false);
    expect(c.map((x) => [x.label, x.days])).toEqual([['出差', 0], ['展会', 0]]);
    expect(countdowns([once], '2026-10-31', false)).toEqual([]); // over
  });
});
