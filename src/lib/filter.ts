import { t as tr } from '@/i18n';
import type { Group, Task } from '@/types';

// 按分组筛选 (GitHub #14), the same rules as the desktop's (native/app/src/desk/vm.rs `shown`,
// `tidy_filter`, `filter_tip`; overlays.rs `filter_rows`). A filter is the list of the group ids
// that show; '' stands for 未分组; an empty list filters nothing. It is for a look: nobody
// saves it.

type Groups = Record<string, Group>;

/** The group a task counts under: its own while that exists, else none (''). */
const groupOf = (groups: Groups, t: Task) => (t.groupId && groups[t.groupId] ? t.groupId : '');

/** Whether the filter lets a task show. */
export function shown(groups: Groups, filter: readonly string[], t: Task): boolean {
  return !filter.length || filter.includes(groupOf(groups, t));
}

/** Groups that are gone leave the filter; every choice ticked is no filter at all. */
export function tidyFilter(groups: Groups, filter: readonly string[]): string[] {
  const kept = [...new Set(filter.filter((g) => g === '' || !!groups[g]))].sort();
  return kept.length > Object.keys(groups).length ? [] : kept;
}

/** A group (or 未分组, '') ticked or unticked. */
export function toggleFilter(groups: Groups, filter: readonly string[], id: string, on: boolean): string[] {
  return tidyFilter(groups, on ? [...filter, id] : filter.filter((g) => g !== id));
}

const byOrder = (groups: Groups) => Object.values(groups).sort((a, b) => a.order - b.order);

/** The names a filter shows, 未分组 first, then the groups in their order. */
export function filterNames(groups: Groups, filter: readonly string[]): string[] {
  const names = filter.includes('') ? [tr("未分组")] : [];
  return names.concat(byOrder(groups).filter((g) => filter.includes(g.id)).map((g) => g.name));
}

/** "按分组筛选：生日、还款" (more than three: "生日、工作、还款 等 5 个分组"); '' when everything shows. */
export function filterTip(groups: Groups, filter: readonly string[]): string {
  if (!filter.length) return '';
  const names = filterNames(groups, filter);
  return tr("按分组筛选：{0}", names.length > 3 ? tr("{0} 等 {1} 个分组", names.slice(0, 3).join('、'), names.length) : names.join('、'));
}

export interface FilterRow {
  id: string;
  name: string;
  /** The group's colour key; none for 未分组. */
  color?: string;
  checked: boolean;
  count: number;
}

/** The list to choose from: 未分组 and the groups in their order, with how many tasks each has. */
export function filterRows(tasks: readonly Task[], groups: Groups, filter: readonly string[]): FilterRow[] {
  const counts = new Map<string, number>();
  for (const t of tasks) counts.set(groupOf(groups, t), (counts.get(groupOf(groups, t)) ?? 0) + 1);
  const row = (id: string, name: string, color?: string): FilterRow => ({ id, name, color, checked: filter.includes(id), count: counts.get(id) ?? 0 });
  return [row('', tr("未分组")), ...byOrder(groups).map((g) => row(g.id, g.name, g.color))];
}

/** The group a new task belongs to: the one group that shows alone (the task would vanish as
 * it is made otherwise). */
export function newTaskGroup(filter: readonly string[]): string | undefined {
  return filter.length === 1 && filter[0] !== '' ? filter[0] : undefined;
}
