import { describe, expect, it } from 'vitest';
import type { Group } from '@/types';
import { filterNames, filterRows, filterTip, newTaskGroup, shown, tidyFilter, toggleFilter } from './filter';
import { mk } from './testing';

const groups: Record<string, Group> = {
  work: { id: 'work', name: '工作', color: 'blue', order: 2 },
  bday: { id: 'bday', name: '生日', color: 'red', order: 1 },
  pay: { id: 'pay', name: '还款', color: 'green', order: 3 },
};
const tasks = [
  mk({ id: 'a', start: '2026-11-20', groupId: 'work' }),
  mk({ id: 'b', start: '2026-11-20', groupId: 'bday' }),
  mk({ id: 'c', start: '2026-11-20' }),
  mk({ id: 'd', start: '2026-11-20', groupId: 'gone' }),
  mk({ id: 'e', start: '2026-11-20', groupId: 'work' }),
];
const ids = (filter: string[]) => tasks.filter((t) => shown(groups, filter, t)).map((t) => t.id).join('');

// The same rules as the desktop's 按分组筛选 (GitHub #14).
describe('filter by group', () => {
  it('shows everything without a filter, else the ticked groups', () => {
    expect(ids([])).toBe('abcde');
    expect(ids(['work'])).toBe('ae');
    expect(ids(['bday', 'work'])).toBe('abe');
    // 未分组: tasks without a group, and those whose group is gone
    expect(ids([''])).toBe('cd');
    expect(ids(['', 'bday'])).toBe('bcd');
  });

  it('drops groups that are gone, and is no filter once every choice is ticked', () => {
    expect(tidyFilter(groups, ['work', 'gone', 'work'])).toEqual(['work']);
    expect(tidyFilter(groups, ['pay', 'bday'])).toEqual(['bday', 'pay']);
    expect(tidyFilter(groups, ['', 'work', 'bday', 'pay'])).toEqual([]);
    expect(tidyFilter(groups, ['work', 'bday', 'pay'])).toEqual(['bday', 'pay', 'work']);
    expect(tidyFilter({}, [''])).toEqual([]);
    expect(tidyFilter(groups, [])).toEqual([]);
  });

  it('ticks and unticks', () => {
    expect(toggleFilter(groups, [], 'work', true)).toEqual(['work']);
    expect(toggleFilter(groups, ['work'], '', true)).toEqual(['', 'work']);
    expect(toggleFilter(groups, ['', 'work'], 'work', false)).toEqual(['']);
    expect(toggleFilter(groups, ['work'], 'work', false)).toEqual([]);
    // the last one of all: everything shows again
    expect(toggleFilter(groups, ['', 'bday', 'pay'], 'work', true)).toEqual([]);
  });

  it('says what it shows', () => {
    expect(filterTip(groups, [])).toBe('');
    expect(filterTip(groups, ['work'])).toBe('按分组筛选：工作');
    // 未分组 first, then the groups in their order
    expect(filterNames(groups, ['work', '', 'bday'])).toEqual(['未分组', '生日', '工作']);
    expect(filterTip(groups, ['work', '', 'bday'])).toBe('按分组筛选：未分组、生日、工作');
    const many = { ...groups, x: { id: 'x', name: '纪念日', color: 'teal', order: 4 }, y: { id: 'y', name: '旅行', color: 'teal', order: 5 } };
    expect(filterTip(many, ['', 'bday', 'work', 'pay', 'x'])).toBe('按分组筛选：未分组、生日、工作 等 5 个分组');
  });

  it('lists the choices with their counts', () => {
    expect(filterRows(tasks, groups, ['work']).map((r) => `${r.name}:${r.count}:${r.checked ? 1 : 0}:${r.color ?? '-'}`)).toEqual([
      '未分组:2:0:-',
      '生日:1:0:red',
      '工作:2:1:blue',
      '还款:0:0:green',
    ]);
  });

  it('gives a new task the one group that shows alone', () => {
    expect(newTaskGroup(['work'])).toBe('work');
    expect(newTaskGroup([''])).toBeUndefined();
    expect(newTaskGroup(['bday', 'work'])).toBeUndefined();
    expect(newTaskGroup([])).toBeUndefined();
  });
});
