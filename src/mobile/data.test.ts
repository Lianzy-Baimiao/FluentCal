import { describe, expect, it } from "vitest";
import { emptyDocument, readDocument } from "./data";
import { planMerge } from "@/lib/export";

const task = {
  id: "t",
  title: "周会",
  start: "2026-11-05",
  end: "2026-11-05",
  repeat: { freq: "weekly", interval: 2, weekdays: [1, 4], count: 9 },
  reminders: [15, 60],
  exDates: ["2026-11-12"],
  doneDates: ["2026-11-05"],
  updatedAt: 100,
};
describe("Android desktop backup compatibility", () => {
  it("keeps advanced repeat rules and fills missing legacy defaults", () => {
    const doc = readDocument(JSON.stringify({ tasks: { t: task } }));
    expect(doc.tasks.t.repeat).toEqual(task.repeat);
    expect(doc.tasks.t.exDates).toEqual(task.exDates);
    expect(doc.tasks.t.reminders).toEqual([15, 60]);
    expect(doc.settings.calendar.weekStart).toBe(1);
    expect(doc.tasks.t.progress).toBe(0);
  });
  it.each([
    { ...task, start: "2026-02-30" },
    { ...task, end: "2026-10-01" },
    { ...task, repeat: { freq: "daily", interval: 0 } },
    { ...task, repeat: { freq: "weekly", weekdays: [8] } },
    { ...task, reminders: [-1] },
    { ...task, notes: {} },
    { ...task, exDates: "bad" },
  ])("rejects unsafe dates/recurrences before any write: %j", (invalid) => {
    expect(() =>
      readDocument(JSON.stringify({ tasks: { t: invalid } })),
    ).toThrow();
  });
  it("rejects incompatible versions, arrays and prototype keys", () => {
    expect(() => readDocument('{"version":2,"tasks":{}}')).toThrow();
    expect(() => readDocument('{"tasks":[]}')).toThrow();
    expect(() => readDocument('{"tasks":{"__proto__":{}}}')).toThrow();
    expect(() =>
      readDocument('{"tasks":{},"holidays":{"2026":{"days":null}}}'),
    ).toThrow();
  });
  it("preserves newer local edits on repeated imports", () => {
    const incoming = readDocument(JSON.stringify({ tasks: { t: task } }));
    const current = emptyDocument();
    current.tasks.t = {
      ...incoming.tasks.t,
      title: "手机修改",
      updatedAt: 101,
    };
    const plan = planMerge(current, incoming);
    expect(plan.tasks).toEqual({});
    expect(plan.kept).toBe(1);
  });
});
