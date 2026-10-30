import type { Group, ISODate, Task } from '@/types';
import { addDays, weekday } from './date';
import { uid } from './uid';

/** First-run sample data: a small project for the Gantt view and a weekly task, all marked
 * `sample` so "清除示例数据" can remove them. How to use the calendar is told by the welcome
 * bar (DeskApp), not by tasks: a task title that long is cut off in a day cell. */
export function sampleData(now: ISODate): { groups: Group[]; tasks: Task[] } {
  const ts = Date.now();
  const g: Group = { id: uid(), name: '示例项目：官网改版', color: 'blue', order: 1, sample: true };
  const base = (p: Partial<Task> & { title: string; start: ISODate }, order: number): Task => ({
    id: uid(),
    end: p.start,
    progress: 0,
    done: false,
    order,
    sample: true,
    createdAt: ts,
    updatedAt: ts,
    ...p,
  });
  const research = base({ title: '需求调研', start: addDays(now, -4), end: addDays(now, -1), groupId: g.id, progress: 100, done: true }, 1);
  const design = base({ title: '视觉设计', start: addDays(now, 0), end: addDays(now, 5), groupId: g.id, progress: 40, color: 'purple', deps: [research.id] }, 2);
  const dev = base({ title: '前端开发', start: addDays(now, 6), end: addDays(now, 16), groupId: g.id, color: 'teal', deps: [design.id] }, 3);
  const test = base({ title: '测试验收', start: addDays(now, 17), end: addDays(now, 21), groupId: g.id, color: 'orange', deps: [dev.id] }, 4);
  const launch = base({ title: '正式上线', start: addDays(now, 22), groupId: g.id, milestone: true, color: 'red', deps: [test.id] }, 5);
  const friday = addDays(now, (5 - weekday(now) + 7) % 7);
  const report = base({ title: '交周报', start: friday, time: '17:00', reminders: [15], repeat: { freq: 'weekly', weekdays: [5] } }, 6);
  return { groups: [g], tasks: [research, design, dev, test, launch, report] };
}
