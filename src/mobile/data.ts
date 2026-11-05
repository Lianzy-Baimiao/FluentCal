import { DEFAULT_SETTINGS, mergeDefaults } from "@/defaults";
import { isISODate } from "@/lib/date";
import type { Doc, Task } from "@/types";

export function emptyDocument(): Doc {
  return {
    version: 1,
    tasks: {},
    groups: {},
    dayMarks: {},
    holidays: {},
    settings: structuredClone(DEFAULT_SETTINGS),
  };
}
function object(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
const frequencies = [
  "daily",
  "weekly",
  "monthly",
  "yearly",
  "workday",
  "legalWorkday",
  "lunarYearly",
  "monthLastDay",
  "ebbinghaus",
];
const safeKey = (key: string) =>
  !["__proto__", "constructor", "prototype"].includes(key);

/** Accept the same JSON backups as desktop, but validate before recurrence/date engines run. */
export function readDocument(text: string): Doc {
  if (text.length > 20_000_000)
    throw new Error("备份超过 20 MB，请先减少历史任务");
  const raw: unknown = JSON.parse(text.replace(/^\uFEFF/, ""));
  if (
    !object(raw) ||
    !object(raw.tasks) ||
    (raw.version !== undefined && raw.version !== 1)
  )
    throw new Error("不是支持的 FluentCal 备份（需要 version 1）");
  const out = emptyDocument();
  for (const [id, v] of Object.entries(raw.tasks)) {
    if (
      !safeKey(id) ||
      !object(v) ||
      typeof v.title !== "string" ||
      !isISODate(v.start) ||
      !isISODate(v.end) ||
      v.end < v.start ||
      v.start < "1901-01-01" ||
      v.end > "2099-12-31"
    )
      throw new Error(`任务 ${id} 的名称或日期无效（支持 1901—2099 年）`);
    if (
      v.time &&
      (typeof v.time !== "string" || !/^([01]\d|2[0-3]):[0-5]\d$/.test(v.time))
    )
      throw new Error(`任务 ${v.title} 的时间无效`);
    if (v.notes !== undefined && typeof v.notes !== "string")
      throw new Error(`任务 ${v.title} 的备注无效`);
    for (const key of ["color", "groupId"])
      if (v[key] !== undefined && typeof v[key] !== "string")
        throw new Error(`任务 ${v.title} 的${key}无效`);
    for (const key of ["doneDates", "exDates"])
      if (
        v[key] !== undefined &&
        (!Array.isArray(v[key]) || !v[key].every(isISODate))
      )
        throw new Error(`任务 ${v.title} 的重复日期无效`);
    if (
      v.deps !== undefined &&
      (!Array.isArray(v.deps) || !v.deps.every((x) => typeof x === "string"))
    )
      throw new Error(`任务 ${v.title} 的依赖无效`);
    if (
      v.reminders !== undefined &&
      (!Array.isArray(v.reminders) ||
        !v.reminders.every((x) => Number.isInteger(x) && x >= 0 && x <= 525600))
    )
      throw new Error(`任务 ${v.title} 的提醒无效`);
    if (v.repeat !== undefined) {
      const r = v.repeat;
      if (
        !object(r) ||
        !frequencies.includes(String(r.freq)) ||
        (r.interval !== undefined &&
          (!Number.isInteger(r.interval) || Number(r.interval) < 1)) ||
        (r.count !== undefined &&
          (!Number.isInteger(r.count) || Number(r.count) < 1)) ||
        (r.until !== undefined && !isISODate(r.until)) ||
        (r.weekdays !== undefined &&
          (!Array.isArray(r.weekdays) ||
            !r.weekdays.every((x) => Number.isInteger(x) && x >= 0 && x <= 6)))
      )
        throw new Error(`任务 ${v.title} 的重复规则无效`);
    }
    out.tasks[id] = {
      ...v,
      id,
      progress:
        typeof v.progress === "number" && Number.isFinite(v.progress)
          ? Math.min(100, Math.max(0, v.progress))
          : 0,
      done: v.done === true,
      order: typeof v.order === "number" ? v.order : 0,
      createdAt: typeof v.createdAt === "number" ? v.createdAt : 0,
      updatedAt: typeof v.updatedAt === "number" ? v.updatedAt : 0,
    } as Task;
  }
  if (raw.groups !== undefined && !object(raw.groups))
    throw new Error("分组数据无效");
  for (const [id, g] of Object.entries(raw.groups ?? {})) {
    if (
      !safeKey(id) ||
      !object(g) ||
      typeof g.name !== "string" ||
      typeof g.color !== "string"
    )
      throw new Error("分组数据无效");
    out.groups[id] = {
      ...g,
      id,
      name: g.name,
      color: g.color,
      order: typeof g.order === "number" ? g.order : 0,
    };
  }
  if (raw.dayMarks !== undefined && !object(raw.dayMarks))
    throw new Error("日期标记无效");
  for (const [date, mark] of Object.entries(raw.dayMarks ?? {})) {
    if (!isISODate(date) || !object(mark) || typeof mark.color !== "string")
      throw new Error("日期标记无效");
    out.dayMarks[date] = { color: mark.color };
  }
  if (raw.holidays !== undefined && !object(raw.holidays))
    throw new Error("节假日数据无效");
  for (const [year, h] of Object.entries(raw.holidays ?? {})) {
    if (
      !safeKey(year) ||
      !object(h) ||
      !Array.isArray(h.days) ||
      !h.days.every(
        (d) =>
          object(d) &&
          isISODate(d.date) &&
          typeof d.name === "string" &&
          typeof d.isOffDay === "boolean",
      )
    )
      throw new Error("节假日数据无效");
    out.holidays[year] = h as unknown as Doc["holidays"][string];
  }
  if (raw.meta !== undefined) {
    if (!object(raw.meta)) throw new Error("同步修改时间无效");
    out.meta = {};
    for (const col of ["tasks", "groups", "dayMarks", "holidays"] as const) {
      const stamps = raw.meta[col];
      if (stamps === undefined) continue;
      if (!object(stamps)) throw new Error("同步修改时间无效");
      out.meta[col] = {};
      for (const [id, time] of Object.entries(stamps)) {
        if (!safeKey(id) || !Number.isSafeInteger(time) || Number(time) < 0)
          throw new Error("同步修改时间无效");
        out.meta[col]![id] = time as number;
      }
    }
  }
  out.settings = mergeDefaults(DEFAULT_SETTINGS, raw.settings);
  return out;
}
