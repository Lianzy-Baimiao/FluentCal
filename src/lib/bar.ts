import { t as tr } from '@/i18n';
import type { ISODate } from '@/types';
import { diffDays, parts } from './date';
import { flexKind } from './flex';
import type { Occurrence } from './recur';
import { doneDay } from './done';

/** What a bar says after its title (设置 › 日历 has a switch for each part). */
export interface BarShow {
  days: boolean;
  progress: boolean;
  done: boolean;
}

/** The day an occurrence on the calendar was ticked done, when that is known (a to-do without
 * a day sits on that day already). */
export function occDoneDay(o: Occurrence): ISODate | undefined {
  return o.done && !flexKind(o.task) ? doneDay(o.task, o.date) : undefined;
}

/** "26.11.27完成": the short form on a bar. */
export function doneShort(day: ISODate): string {
  const [y, m, d] = parts(day);
  return tr("{0}.{1}.{2}完成", String(((y % 100) + 100) % 100).padStart(2, '0'), m, d);
}

/**
 * What a bar says after its title, the same as the desktop's (native/app/src/desk/vm.rs
 * `bar_label`), for a task over several days:
 * - "第5-2天（40%）" (GitHub #13): 5 days in all, today is the 2nd, 40 % of the time is gone.
 *   Counted from today: 0 before it begins, all of them once it is over.
 * - "进度 60%" (GitHub #10, #13): the task's own progress, 0 % and 100 % included. Not for
 *   repeating tasks (each time has no progress of its own).
 * - "26.11.27完成" (GitHub #15): the day it was ticked done, standing for "进度 100%".
 *
 * `fill`: the done part of this piece of the bar, 0–1 (the piece begins on `segStart` and is
 * `segDays` long), drawn darker.
 */
export function barLabel(o: Occurrence, segStart: ISODate, segDays: number, today: ISODate, show: BarShow): { text: string; fill: number } {
  const task = o.task;
  if (o.start === o.end || task.milestone) return { text: '', fill: 0 };
  const total = Math.max(1, diffDays(o.start, o.end) + 1);
  const out: string[] = [];
  if (show.days) {
    const gone = Math.min(total, Math.max(0, diffDays(o.start, today) + 1));
    out.push(tr("第{0}-{1}天（{2}%）", total, gone, Math.round((gone * 100) / total)));
  }
  let fill = 0;
  const day = show.done ? occDoneDay(o) : undefined;
  if (day) {
    out.push(doneShort(day));
  } else if (show.progress && !task.repeat) {
    const p = task.done ? 100 : Math.min(100, Math.max(0, task.progress));
    out.push(tr("进度 {0}%", p));
    if (!task.done) {
      const before = Math.max(0, diffDays(o.start, segStart));
      fill = Math.min(1, Math.max(0, ((total * p) / 100 - before) / Math.max(1, segDays)));
    }
  }
  return { text: out.join(' '), fill };
}
