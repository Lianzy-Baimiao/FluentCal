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
});
