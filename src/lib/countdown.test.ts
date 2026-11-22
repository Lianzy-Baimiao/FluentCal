import { describe, expect, it } from 'vitest';
import { countdowns } from './countdown';
import type { Task } from '@/types';

const task = (id: string, groupId?: string, countdown = false): Task => ({ id, title: id, groupId, countdown, start: '2026-11-25', end: '2026-11-25', progress: 0, done: false, order: 1, createdAt: 0, updatedAt: 0 });
describe('countdown sources', () => {
  it('unions marked tasks and groups, and keeps an empty selection empty', () => {
    const tasks = [task('a', 'birthday', true), task('b', 'birthday'), task('c', undefined, true)];
    const ids = (marked: boolean, groups: string[]) => countdowns(tasks, '2026-11-21', false, marked, groups).map(c => c.key);
    expect(ids(true, [])).toEqual(['a', 'c']);
    expect(ids(false, ['birthday'])).toEqual(['a', 'b']);
    expect(ids(true, ['birthday'])).toEqual(['a', 'b', 'c']);
    expect(ids(false, [])).toEqual([]);
  });

  it('adds independently selected festival and solar-term sources', () => {
    const festivals = countdowns([], '2026-11-03', false, false, [], true, false);
    const terms = countdowns([], '2026-11-03', false, false, [], false, true);
    expect(festivals.length).toBeGreaterThan(1);
    expect(festivals.every(item => item.key.startsWith('festival:'))).toBe(true);
    expect(terms.length).toBeGreaterThan(10);
    expect(terms.every(item => item.key.startsWith('term:'))).toBe(true);
    expect(countdowns([], '2026-11-03', false, false, [])).toEqual([]);
  });

  it('keeps the statutory holiday-only legacy selection to one item', () => {
    expect(countdowns([], '2026-09-20', true, false, [], false, false).map(item => item.date)).toEqual(['2026-09-25']);
  });

  it('unions observances on the same day and treats ungrouped as opt-in', () => {
    const sameDay = countdowns([], '2026-04-04', true, false, [], false, true).filter(item => item.date === '2026-04-05');
    expect(sameDay).toHaveLength(1);
    expect(sameDay[0]?.label).toContain('清明');
    expect(countdowns([task('u')], '2026-11-21', false, false, [])).toEqual([]);
    expect(countdowns([task('u')], '2026-11-21', false, false, ['']).map(item => item.key)).toEqual(['u']);
  });
});
