import type { ISODate, Task } from '@/types';

let n = 0;

/** Minimal task for unit tests. */
export function mk(p: Partial<Task> & { start: ISODate }): Task {
  n++;
  return {
    id: p.id ?? `t${n}`,
    title: p.title ?? `任务${n}`,
    end: p.end ?? p.start,
    progress: 0,
    done: false,
    order: n,
    createdAt: 0,
    updatedAt: 0,
    ...p,
  };
}
