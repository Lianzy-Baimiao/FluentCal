import { describe, expect, it } from 'vitest';
import type { Task } from '@/types';
import { agenda, allTasks, dayHeading } from './agenda';

const t = (id: string, start: string, end = start, extra: Partial<Task> = {}): Task => ({
  id, title: id, start, end, progress: 0, done: false, order: 1, createdAt: 0, updatedAt: 0, ...extra,
});

describe('agenda', () => {
  const today = '2026-10-30';

  it('groups by day, keeps today, sorts all-day first then by time', () => {
    const days = agenda(
      [t('late', '2026-10-31', '2026-10-31', { time: '18:00' }), t('early', '2026-10-31', '2026-10-31', { time: '08:30' }), t('allday', '2026-10-31')],
      today, '2026-11-05', today,
    );
    expect(days.map((d) => d.date)).toEqual(['2026-10-30', '2026-10-31']);
    expect(days[0].items).toEqual([]);
    expect(days[1].items.map((o) => o.task.id)).toEqual(['allday', 'early', 'late']);
  });

  it('lists a running multi-day task on the first day and repeats on each day', () => {
    const days = agenda(
      [t('trip', '2026-10-28', '2026-11-02'), t('gym', '2026-10-27', '2026-10-27', { repeat: { freq: 'weekly', weekdays: [2, 4] } })],
      today, '2026-11-06', today,
    );
    expect(days.find((d) => d.date === today)?.items.map((o) => o.task.id)).toEqual(['trip']);
    expect(days.filter((d) => d.items.some((o) => o.task.id === 'gym')).map((d) => d.date)).toEqual(['2026-11-03', '2026-11-05']);
  });

  it('can hide done tasks', () => {
    const tasks = [t('done', today, today, { done: true }), t('open', today)];
    expect(agenda(tasks, today, today, today, false)[0].items.map((o) => o.task.id)).toEqual(['open']);
    expect(agenda(tasks, today, today, today)[0].items.map((o) => o.task.id)).toEqual(['open', 'done']);
  });

  it('names days, with the year when it differs', () => {
    expect([dayHeading(today, today), dayHeading('2026-10-31', today), dayHeading('2026-10-29', today), dayHeading('2026-11-02', today)]).toEqual(['今天', '明天', '昨天', '11月2日']);
    expect([dayHeading('2025-03-04', today), dayHeading('2027-01-01', today)]).toEqual(['2025年3月4日', '2027年1月1日']);
  });
});

describe('allTasks (所有)', () => {
  const today = '2026-10-30';
  const tasks = [
    t('later', '2027-02-19'),
    t('daily', '2026-01-05', '2026-01-05', { repeat: { freq: 'daily', interval: 1 } }),
    t('old', '2025-03-04', '2025-03-06', { done: true }),
    t('birthday', '1992-12-21', '1992-12-21', { repeat: { freq: 'lunarYearly' }, doneDates: ['1992-12-21'] }),
  ];

  it('lists every task once on its start date, past ones too, and keeps today', () => {
    const days = allTasks(tasks, today);
    expect(days.map((d) => d.date)).toEqual(['1992-12-21', '2025-03-04', '2026-01-05', today, '2027-02-19']);
    expect(days.reduce((n, d) => n + d.items.length, 0)).toBe(4);
    expect(days[1].items[0]).toMatchObject({ end: '2025-03-06', done: true });
    expect(days[0].items[0].done).toBe(false);
    expect(days[3].items).toEqual([]);
  });

  it('hides done tasks but not a series', () => {
    const ids = allTasks(tasks, today, false).flatMap((d) => d.items.map((o) => o.task.id));
    expect(ids).toEqual(['birthday', 'daily', 'later']);
  });
});
