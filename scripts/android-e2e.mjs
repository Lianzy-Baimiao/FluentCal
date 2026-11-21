import { preview } from "vite";
import { chromium } from "playwright-core";
import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";

const server = await preview({
  mode: "android",
  preview: { port: 1437, strictPort: true },
  logLevel: "error",
});
const browser = await chromium.launch({ channel: "msedge", headless: true });
const context = await browser.newContext({
  viewport: { width: 390, height: 844 },
  isMobile: true,
  hasTouch: true,
  locale: "zh-CN",
  timezoneId: "Asia/Shanghai",
});
const page = await context.newPage();
page.setDefaultTimeout(8000);
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
const key = "CapacitorStorage.fluentcal.mobile.doc.v1";
const stored = () =>
  page.evaluate((key) => JSON.parse(localStorage.getItem(key) ?? "{}"), key);
const dialog = () => page.getByRole("dialog");
const nav = (name) => page.locator(".bottom-nav").getByRole("button", { name });
const save = async () => {
  await dialog().getByRole("button", { name: "保存", exact: true }).click();
  await dialog().waitFor({ state: "hidden" });
};
const today = new Date().toLocaleDateString("sv-SE", {
  timeZone: "Asia/Shanghai",
});
const check = async (name, fn) => {
  await fn();
  console.log(`PASS ${name}`);
};

try {
  await page.goto("http://localhost:1437");
  await page.locator(".task-card").first().waitFor();
  await check(
    "starts in mobile mode with persistent isolated storage and SVG icons",
    async () => {
      assert(Object.keys((await stored()).tasks).length >= 6);
      assert.equal(await page.locator(".bottom-nav svg").count(), 4);
      assert.equal(
        await page.evaluate(() => localStorage.getItem("fluentcal.mock.doc")),
        null,
      );
    },
  );
  await check("Today opens the current calendar from every tab and clears search", async () => {
    for (const name of ["日程", "甘特", "设置", "月历"]) {
      await nav(name).click();
      if (name !== "设置") {
        await page.locator(".app-bar").getByRole("button", { name: "搜索", exact: true }).click();
        await page.getByPlaceholder("搜索任务、备注…").fill("不应继续过滤今天的任务");
      }
      if (name === "月历") {
        await page.locator(".calendar-toolbar").getByRole("button", { name: "›", exact: true }).click();
      }
      await page.locator(".app-bar").getByRole("button", { name: /今天/ }).click();
      await page.locator(".calendar-page").waitFor({ state: "visible", timeout: 3000 });
      assert.equal(await page.locator(".month-day.today.selected").count(), 1);
      assert((await page.locator(".month-day.today.selected").getAttribute("aria-label")).startsWith(today));
      assert.equal(await page.locator(".search-row").count(), 0);
      assert.equal(await page.locator(".toast").innerText(), "已回到今天");
    }
    await nav("日程").click();
  });
  await check("creates, reloads and completes a task", async () => {
    await page.locator(".primary-round").click();
    await dialog().getByLabel("任务名称").fill("安卓离线任务");
    await dialog().getByLabel("备注", { exact: true }).fill("手机编辑的内容");
    await dialog().getByLabel("提醒", { exact: true }).selectOption("[15]");
    await save();
    await page.reload();
    await page
      .getByRole("button", { name: "完成 安卓离线任务", exact: true })
      .click();
    await page
      .getByRole("button", { name: "取消完成 安卓离线任务", exact: true })
      .waitFor();
    assert(
      Object.values((await stored()).tasks).find(
        (t) => t.title === "安卓离线任务",
      ).done,
    );
  });
  await check("discarding an edit does not save progress or text", async () => {
    await page.locator(".task-main", { hasText: "安卓离线任务" }).click();
    await dialog().getByLabel("任务名称").fill("应当放弃的修改");
    await dialog().getByRole("button", { name: "关闭", exact: true }).click();
    await dialog().getByRole("button", { name: "放弃", exact: true }).click();
    assert(
      !Object.values((await stored()).tasks).some(
        (t) => t.title === "应当放弃的修改",
      ),
    );
  });
  await check(
    "calendar selection creates on the selected day and Gantt opens a task",
    async () => {
      await nav("月历").click();
      assert.equal(await page.locator(".month-day").count(), 42);
      await page.locator(".month-day:not(.dim)").nth(10).click();
      await page.getByRole("button", { name: "添加任务", exact: true }).click();
      await dialog().getByLabel("任务名称").fill("月历创建");
      assert(
        (
          await dialog().getByLabel("开始", { exact: true }).inputValue()
        ).endsWith("-11"),
      );
      await save();
      await nav("甘特").click();
      await page.locator(".mg .task-name", { hasText: "视觉设计" }).click();
      await dialog().getByLabel("备注", { exact: true }).fill("从甘特图编辑");
      await save();
      assert(
        Object.values((await stored()).tasks).some(
          (t) => t.notes === "从甘特图编辑",
        ),
      );
    },
  );
  await check(
    "desktop backup import previews, merges, validates, and preserves advanced recurrence",
    async () => {
      await nav("设置").click();
      const task = {
        id: "desktop-repeating",
        title: "桌面重复规则",
        start: today,
        end: today,
        done: false,
        progress: 0,
        order: 1,
        createdAt: 1,
        updatedAt: 1,
        repeat: {
          freq: "weekly",
          interval: 2,
          weekdays: [0, 1, 2, 3, 4, 5, 6],
          count: 12,
        },
        reminders: [10, 60],
        exDates: ["2026-01-01"],
        doneDates: [],
      };
      const backup = {
        version: 1,
        tasks: { [task.id]: task },
        groups: {},
        settings: { theme: { mode: "dark" } },
      };
      await page
        .locator("input[type=file]")
        .setInputFiles({
          name: "desktop.json",
          mimeType: "application/json",
          buffer: Buffer.from(JSON.stringify(backup)),
        });
      await page.getByRole("dialog", { name: "合并备份" }).waitFor();
      assert.equal((await stored()).tasks[task.id], undefined);
      await page.getByRole("button", { name: "确认合并", exact: true }).click();
      await page.getByRole("dialog").waitFor({ state: "hidden" });
      const data = await stored();
      assert(data.tasks[task.id]);
      assert.notEqual(data.settings.theme.mode, "dark");
      assert(Object.values(data.tasks).some((t) => t.title === "安卓离线任务"));
      await nav("日程").click();
      await page.locator(".task-main", { hasText: task.title }).first().click();
      await dialog().getByLabel("备注", { exact: true }).fill("只改备注");
      await save();
      const edited = (await stored()).tasks[task.id];
      assert.deepEqual(edited.repeat, task.repeat);
      assert.deepEqual(edited.reminders, [10, 60]);
      assert.deepEqual(edited.exDates, task.exDates);
      await nav("设置").click();
      const beforeInvalid = (await stored()).tasks;
      await page
        .locator("input[type=file]")
        .setInputFiles({
          name: "bad.json",
          mimeType: "application/json",
          buffer: Buffer.from(
            '{"tasks":{"broken":{"title":"坏数据","start":"bad","end":"bad"}}}',
          ),
        });
      await page.getByRole("alert").waitFor();
      assert.deepEqual((await stored()).tasks, beforeInvalid);
      assert(!(await stored()).tasks.broken);
      await page
        .getByRole("alert")
        .getByRole("button", { name: "关闭" })
        .click();
    },
  );
  await check("JSON export downloads a compatible document", async () => {
    const downloading = page.waitForEvent("download");
    await page.getByRole("button", { name: /导出备份/ }).click();
    const download = await downloading;
    assert(download.suggestedFilename().endsWith(".json"));
    const stream = await download.createReadStream();
    const chunks = [];
    for await (const chunk of stream) chunks.push(chunk);
    assert(
      Object.keys(JSON.parse(Buffer.concat(chunks).toString()).tasks).length >=
        7,
    );
  });
  await check(
    "deleting and undo restore the saved task",
    async () => {
      await nav("日程").click();
      await page.locator(".task-main", { hasText: "安卓离线任务" }).click();
      await dialog()
        .getByRole("button", { name: "删除任务", exact: true })
        .click();
      await dialog()
        .getByRole("button", { name: "确认删除", exact: true })
        .click();
      await dialog().waitFor({ state: "hidden" });
      assert(
        !Object.values((await stored()).tasks).some(
          (t) => t.title === "安卓离线任务",
        ),
      );
      await page.getByRole("button", { name: "撤销", exact: true }).click();
      await page.locator(".task-main", { hasText: "安卓离线任务" }).waitFor();
      await page.waitForFunction(key => Object.values(JSON.parse(localStorage.getItem(key)).tasks).some(t => t.title === '安卓离线任务'), key);
      assert(
        Object.values((await stored()).tasks).some(
          (t) => t.title === "安卓离线任务",
        ),
      );
    },
  );
  // 0.3.3: what the desktop added in 0.3.10 / 0.3.11
  const byTitle = async (title) => Object.values((await stored()).tasks).find((t) => t.title === title);
  const waitTask = (title, test) =>
    page.waitForFunction(
      ([key, title, test]) => {
        const t = Object.values(JSON.parse(localStorage.getItem(key)).tasks).find((t) => t.title === title);
        return new Function("t", `return ${test}`)(t);
      },
      [key, title, test],
    );
  await check("to-dos without a day: add, tick on today, untick, and they keep their dates when edited", async () => {
    await nav("日程").click();
    const card = page.getByRole("region", { name: "不定日待办" });
    // one line until it is opened, so the day's tasks stay near the top
    assert.equal(await card.getByLabel("添加每月待办").count(), 0);
    assert((await card.boundingBox()).height <= 50, "the folded card is one line");
    await card.getByRole("button", { name: "展开不定日待办", exact: true }).click();
    await card.getByLabel("添加每月待办").fill("洗车");
    await card.getByLabel("添加每月待办").press("Enter");
    await card.getByLabel("添加无日期待办").fill("换护照");
    await card.getByRole("button", { name: "添加", exact: true }).click();
    await waitTask("换护照", "!!t");
    const car = await byTitle("洗车");
    assert.equal(car.flex, "month");
    assert.equal(car.start, today.slice(0, 8) + "01");
    assert.equal((await byTitle("换护照")).flex, "someday");
    assert.equal(await card.getByLabel("添加每月待办").inputValue(), "");
    // ticking records today; the to-do stays in the list, struck through, and is on today's agenda
    await card.getByRole("button", { name: "完成 洗车", exact: true }).click();
    await waitTask("洗车", "!!t.flexDone");
    assert.deepEqual((await byTitle("洗车")).flexDone, { [today.slice(0, 7)]: today });
    assert.equal((await byTitle("洗车")).done, false);
    await card.locator(".flex-item.done", { hasText: "洗车" }).waitFor();
    await page.locator(".task-card.done", { hasText: "洗车" }).waitFor();
    // unticked from the day it was done on
    await page.locator(".task-card").getByRole("button", { name: "取消完成 洗车", exact: true }).click();
    await waitTask("洗车", "!t.flexDone");
    await card.getByRole("button", { name: "完成 洗车", exact: true }).waitFor();
    // a to-do without a date is done as a whole and leaves the list
    await card.getByRole("button", { name: "完成 换护照", exact: true }).click();
    await waitTask("换护照", "t.done === true");
    assert.deepEqual((await byTitle("换护照")).flexDone, { "": today });
    assert.equal(await card.locator(".flex-item", { hasText: "换护照" }).count(), 0);
    // the editor has no dates for it and leaves them alone
    await card.locator(".flex-title", { hasText: "洗车" }).click();
    assert.equal(await dialog().getByLabel("开始", { exact: true }).count(), 0);
    assert.equal(await dialog().getByLabel("重复", { exact: true }).count(), 0);
    await dialog().getByLabel("任务名称").fill("洗车和打蜡");
    await save();
    const renamed = await byTitle("洗车和打蜡");
    assert.deepEqual([renamed.flex, renamed.start, renamed.end, renamed.done], ["month", car.start, car.start, false]);
    // not a bar on its start day in the Gantt chart: the whole month while it is open
    // folded: the line says what is left; the choice is kept across a restart
    await card.getByRole("button", { name: "收起不定日待办", exact: true }).click();
    assert.equal(await card.locator(".flex-item").count(), 0);
    assert.match(await card.innerText(), /1 项未完成/);
    await card.getByRole("button", { name: "展开不定日待办", exact: true }).click();
    await page.reload();
    await card.locator(".flex-item", { hasText: "洗车和打蜡" }).waitFor();
    await card.getByRole("button", { name: "收起不定日待办", exact: true }).click();
    await page.reload();
    await card.getByRole("button", { name: "展开不定日待办", exact: true }).waitFor();
    assert.equal(await card.locator(".flex-item").count(), 0);
    await nav("甘特").click();
    await page.locator(".mg .task-name", { hasText: "洗车和打蜡" }).waitFor();
    assert.match(await page.locator(".mg .task-name", { hasText: "洗车和打蜡" }).innerText(), /每月一次/);
    assert.equal(await page.locator(".mg .task-name", { hasText: "换护照" }).count(), 0);
    await nav("日程").click();
  });
  await check("every N days can be set, and a daily task with an end date is once a day", async () => {
    await page.locator(".primary-round").click();
    await dialog().getByLabel("任务名称").fill("背单词");
    await dialog().getByLabel("重复", { exact: true }).selectOption("daily");
    await dialog().getByLabel("每隔几天").fill("2");
    const end = new Date(Date.parse(today) + 4 * 86400000).toISOString().slice(0, 10);
    await dialog().getByLabel("结束", { exact: true }).fill(end);
    await dialog().getByText("每次占一天").waitFor();
    await save();
    const t = await byTitle("背单词");
    assert.deepEqual(t.repeat, { freq: "daily", interval: 2 });
    assert.equal(t.end, end);
    // today, +2, +4: three single days, each with its own tick
    assert.equal(await page.locator(".task-card", { hasText: "背单词" }).count(), 3);
    // reopening keeps the interval and the rule object
    await page.locator(".task-main", { hasText: "背单词" }).first().click();
    assert.equal(await dialog().getByLabel("每隔几天").inputValue(), "2");
    await dialog().getByLabel("备注", { exact: true }).fill("每次 20 个");
    await save();
    assert.deepEqual((await byTitle("背单词")).repeat, { freq: "daily", interval: 2 });
  });
  // 0.3.5: what the desktop added in 0.3.14 and 0.3.17
  await mkdir("shots/android", { recursive: true });
  const plus = (n) => new Date(Date.parse(today) + n * 86400000).toISOString().slice(0, 10);
  const md = (iso) => `${Number(iso.slice(5, 7))}月${Number(iso.slice(8, 10))}日`;
  await check("the day a task is ticked done is recorded, shown on its card and can be corrected", async () => {
    await nav("日程").click();
    // 视觉设计 runs from today for six days: its card says which day it is
    const card = page.locator(".task-card", { hasText: "视觉设计" }).first();
    assert.match(await card.innerText(), /第6-1天（17%）/);
    assert.equal((await byTitle("视觉设计")).doneOn, undefined);
    await card.getByRole("button", { name: "完成 视觉设计", exact: true }).click();
    await waitTask("视觉设计", `t.done === true && t.doneOn === "${today}"`);
    await page.locator(".task-card.done", { hasText: "视觉设计" }).filter({ hasText: `${md(today)}完成` }).waitFor();
    // 完成日期 in the editor: an earlier day, never a later one
    await page.locator(".task-main", { hasText: "视觉设计" }).first().click();
    assert.equal(await dialog().getByLabel("完成日期").inputValue(), today);
    await dialog().getByLabel("完成日期").fill(plus(1));
    await dialog().getByRole("button", { name: "保存", exact: true }).click();
    await dialog().getByRole("alert").filter({ hasText: "完成日期不能晚于今天" }).waitFor();
    assert.equal((await byTitle("视觉设计")).doneOn, today);
    await dialog().getByLabel("完成日期").fill(plus(-1));
    await dialog().getByLabel("完成日期").scrollIntoViewIfNeeded();
    await page.screenshot({ path: "shots/android/editor-done.png", animations: "disabled" });
    await save();
    await waitTask("视觉设计", `t.doneOn === "${plus(-1)}"`);
    await page.locator(".task-card.done", { hasText: "视觉设计" }).filter({ hasText: `${md(plus(-1))}完成` }).waitFor();
    // another edit leaves the day alone
    await page.locator(".task-main", { hasText: "视觉设计" }).first().click();
    assert.equal(await dialog().getByLabel("完成日期").inputValue(), plus(-1));
    await dialog().getByLabel("备注", { exact: true }).fill("改了备注");
    await save();
    await waitTask("视觉设计", `t.notes === "改了备注"`);
    assert.equal((await byTitle("视觉设计")).doneOn, plus(-1));
    // the day list of the month page says it too
    await nav("月历").click();
    await page.locator(".month-day.today").click();
    await page.locator(".compact-task", { hasText: "视觉设计" }).filter({ hasText: `${md(plus(-1))}完成` }).waitFor();
    await page.screenshot({ path: "shots/android/calendar-done.png" });
    await nav("日程").click();
    await page.screenshot({ path: "shots/android/agenda-done.png" });
    // unticked: no day at all; ticked again: today, whatever was set by hand
    await page.getByRole("button", { name: "取消完成 视觉设计", exact: true }).first().click();
    await waitTask("视觉设计", `t.done === false && !("doneOn" in t)`);
    await page.getByRole("button", { name: "完成 视觉设计", exact: true }).first().click();
    await waitTask("视觉设计", `t.doneOn === "${today}"`);
    // unticked in the editor: the field goes, and so does the day
    await page.locator(".task-main", { hasText: "视觉设计" }).first().click();
    await dialog().getByLabel("已完成").uncheck();
    assert.equal(await dialog().getByLabel("完成日期").count(), 0);
    await save();
    await waitTask("视觉设计", `t.done === false && !("doneOn" in t)`);
    // ticked in the editor and the date emptied: today all the same
    await page.locator(".task-main", { hasText: "视觉设计" }).first().click();
    await dialog().getByLabel("已完成").check();
    await dialog().getByLabel("完成日期").fill("");
    await dialog().getByText("不填则记为今天完成。").waitFor();
    await save();
    await waitTask("视觉设计", `t.done === true && t.doneOn === "${today}"`);
    // emptied on a task that was done: no day (as for one done before days were recorded)
    await page.locator(".task-main", { hasText: "视觉设计" }).first().click();
    await dialog().getByLabel("完成日期").fill("");
    await dialog().getByText("没有记录是哪天完成的，可以在这里补上。").waitFor();
    await save();
    await waitTask("视觉设计", `t.done === true && !t.doneOn`);
    await page.locator(".task-main", { hasText: "视觉设计" }).first().click();
    await dialog().getByLabel("已完成").uncheck();
    await save();
    await waitTask("视觉设计", `t.done === false && !("doneOn" in t)`);
    // each time of a repeating task has its own day
    await page.locator(".task-card", { hasText: "交周报" }).first().getByRole("button", { name: "完成 交周报", exact: true }).click();
    await waitTask("交周报", `(t.doneDates ?? []).length === 1 && !!t.doneOnDates`);
    const report = await byTitle("交周报");
    assert.deepEqual(report.doneOnDates, { [report.doneDates[0]]: today });
    assert.deepEqual([report.done, "doneOn" in report], [false, false]);
    await page.locator(".task-card.done", { hasText: "交周报" }).filter({ hasText: `${md(today)}完成` }).waitFor();
    await page.getByRole("button", { name: "取消完成 交周报", exact: true }).click();
    await waitTask("交周报", `!t.doneOnDates && t.doneDates.length === 0`);
    // a new task made done is done today
    await page.locator(".primary-round").click();
    await dialog().getByLabel("任务名称").fill("补记的任务");
    await dialog().getByLabel("已完成").check();
    assert.equal(await dialog().getByLabel("完成日期").inputValue(), today);
    await save();
    await waitTask("补记的任务", `!!t && t.done === true && t.doneOn === "${today}"`);
  });
  await check("filter by group: the agenda, the to-do card and the month follow it, search does not", async () => {
    await nav("日程").click();
    const bar = page.locator(".app-bar");
    const group = "示例项目：官网改版";
    const groupId = Object.values((await stored()).groups).find((g) => g.name === group).id;
    const titles = async () => [...new Set(await page.locator(".task-card .task-main strong").allInnerTexts())];
    const count = async () => Number((await page.locator(".hero h1").innerText()).match(/\d+/)[0]);
    const todayCount = async () => Number((await page.locator(".month-day.today").getAttribute("aria-label")).match(/(\d+) 项任务/)[1]);
    const all = await titles();
    const allCount = await count();
    assert(all.includes("交周报") && all.includes("前端开发") && all.includes("背单词"), all.join());
    await bar.getByRole("button", { name: "按分组筛选", exact: true }).click();
    const sheet = page.getByRole("dialog", { name: "按分组筛选" });
    assert.equal(await sheet.getByRole("checkbox", { name: /全部/ }).getAttribute("aria-checked"), "true");
    assert.equal(await sheet.getByRole("checkbox").count(), 3, "全部, 未分组 and the one group");
    await sheet.getByRole("checkbox", { name: new RegExp(group) }).click();
    assert.equal(await sheet.getByRole("checkbox", { name: /全部/ }).getAttribute("aria-checked"), "false");
    await page.screenshot({ path: "shots/android/filter.png", animations: "disabled" });
    await sheet.getByRole("button", { name: "完成", exact: true }).click();
    await sheet.waitFor({ state: "hidden" });
    // only the group's tasks, and the line under the bar says which
    const some = await titles();
    assert(some.includes("前端开发") && !some.includes("交周报") && !some.includes("背单词") && !some.includes("安卓离线任务"), some.join());
    assert.equal(await page.locator(".filter-strip span").innerText(), `只显示：${group}`);
    assert.equal(await bar.getByRole("button", { name: `按分组筛选：${group}`, exact: true }).getAttribute("aria-pressed"), "true");
    assert((await count()) < allCount, "the headline counts what the page shows");
    // (the badge on 日程 still counts everything)
    assert.match(await nav(/日程/).getAttribute("aria-label"), new RegExp(`${allCount} 个未完成任务`));
    // the to-dos without a day have no group
    const card = page.getByRole("region", { name: "不定日待办" });
    assert.match(await card.innerText(), /没有未完成/);
    await page.screenshot({ path: "shots/android/agenda-filter.png" });
    await page.setViewportSize({ width: 320, height: 700 });
    await page.screenshot({ path: "shots/android/agenda-filter-320.png" });
    await page.setViewportSize({ width: 390, height: 844 });
    // a search looks through everything, and the line is gone meanwhile
    await bar.getByRole("button", { name: "搜索", exact: true }).click();
    await page.getByPlaceholder("搜索任务、备注…").fill("交周报");
    await page.locator(".task-card", { hasText: "交周报" }).waitFor();
    assert.equal(await page.locator(".filter-strip").count(), 0);
    await page.locator(".search-row").getByRole("button").click();
    await page.locator(".filter-strip").waitFor();
    // a new task joins the one group that shows
    await page.locator(".primary-round").click();
    assert.equal(await dialog().getByLabel("分组", { exact: true }).inputValue(), groupId);
    await dialog().getByLabel("任务名称").fill("筛选时新建");
    await save();
    await waitTask("筛选时新建", `!!t && t.groupId === "${groupId}"`);
    await page.locator(".task-card", { hasText: "筛选时新建" }).waitFor();
    assert.equal(await page.locator(".toast").innerText(), "任务已保存");
    // … and one moved out of it is said to be out of sight
    await page.locator(".task-main", { hasText: "筛选时新建" }).click();
    await dialog().getByLabel("分组", { exact: true }).selectOption("");
    await save();
    await page.locator(".toast", { hasText: "任务已保存（当前筛选下不显示）" }).waitFor();
    assert.equal(await page.locator(".task-card", { hasText: "筛选时新建" }).count(), 0);
    // the month: the day's count and its list
    await nav("月历").click();
    await page.locator(".filter-strip").waitFor();
    await page.locator(".month-day.today").click();
    const few = await todayCount();
    assert.equal(await page.locator(".compact-task", { hasText: "筛选时新建" }).count(), 0);
    // the Gantt chart has its own filter: no line, no button
    await nav("甘特").click();
    assert.equal(await page.locator(".filter-strip").count(), 0);
    assert.equal(await bar.getByRole("button", { name: /按分组筛选/ }).count(), 0);
    // 未分组 as well: every choice ticked is no filter at all
    await nav("月历").click();
    await bar.getByRole("button", { name: /按分组筛选/ }).click();
    await sheet.getByRole("checkbox", { name: /未分组/ }).click();
    assert.equal(await sheet.getByRole("checkbox", { name: /全部/ }).getAttribute("aria-checked"), "true");
    await sheet.getByRole("button", { name: "完成", exact: true }).click();
    await sheet.waitFor({ state: "hidden" });
    assert.equal(await page.locator(".filter-strip").count(), 0);
    assert((await todayCount()) > few, "today has tasks without a group too");
    await page.locator(".compact-task", { hasText: "筛选时新建" }).waitFor();
    // 未分组 alone, then 显示全部 on the line
    await bar.getByRole("button", { name: "按分组筛选", exact: true }).click();
    await sheet.getByRole("checkbox", { name: /未分组/ }).click();
    await sheet.getByRole("button", { name: "完成", exact: true }).click();
    assert.equal(await page.locator(".filter-strip span").innerText(), "只显示：未分组");
    await nav("日程").click();
    const loose = await titles();
    assert(loose.includes("交周报") && !loose.includes("前端开发"), loose.join());
    await page.locator(".filter-strip").getByRole("button", { name: "显示全部", exact: true }).click();
    assert.equal(await page.locator(".filter-strip").count(), 0);
    assert((await titles()).includes("前端开发"));
    // it is for a look: gone after a restart
    await bar.getByRole("button", { name: "按分组筛选", exact: true }).click();
    await sheet.getByRole("checkbox", { name: new RegExp(group) }).click();
    await sheet.getByRole("button", { name: "完成", exact: true }).click();
    await page.locator(".filter-strip").waitFor();
    await page.reload();
    await page.locator(".task-card").first().waitFor();
    assert.equal(await page.locator(".filter-strip").count(), 0);
  });
  await check("a backup with a completion day that is not a day is taken without it", async () => {
    await nav("设置").click();
    const base = { start: today, end: today, done: true, progress: 100, order: 1, createdAt: 1, updatedAt: 1 };
    const backup = {
      version: 1,
      tasks: {
        "done-good": { ...base, id: "done-good", title: "有完成日期", doneOn: plus(-3) },
        "done-bad": { ...base, id: "done-bad", title: "完成日期不对", doneOn: "昨天", doneOnDates: { x: 1 } },
        "done-rep": { ...base, id: "done-rep", title: "重复的完成日期", done: false, progress: 0, repeat: { freq: "daily" }, doneDates: [today], doneOnDates: { [today]: plus(-1), bad: "x" } },
      },
      groups: {},
    };
    await page.locator("input[type=file]").setInputFiles({ name: "done.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(backup)) });
    await page.getByRole("dialog", { name: "合并备份" }).waitFor();
    await page.getByRole("button", { name: "确认合并", exact: true }).click();
    await page.getByRole("dialog").waitFor({ state: "hidden" });
    const tasks = (await stored()).tasks;
    assert.equal(tasks["done-good"].doneOn, plus(-3));
    assert.deepEqual(["doneOn" in tasks["done-bad"], "doneOnDates" in tasks["done-bad"]], [false, false]);
    assert.deepEqual(tasks["done-rep"].doneOnDates, { [today]: plus(-1) });
    await nav("日程").click();
    await page.locator(".task-card.done", { hasText: "有完成日期" }).filter({ hasText: `${md(plus(-3))}完成` }).waitFor();
    assert.doesNotMatch(await page.locator(".task-card", { hasText: "完成日期不对" }).innerText(), /日完成/);
  });
  await page.locator('.toast').waitFor({ state: 'hidden' });
  await check(
    "phone/tablet layouts have no horizontal page overflow and the editor scrolls",
    async () => {
      for (const size of [
        { width: 360, height: 800 },
        { width: 844, height: 390 },
        { width: 768, height: 1024 },
      ]) {
        await page.setViewportSize(size);
        for (const name of ["日程", "月历", "甘特", "设置"]) {
          await nav(name).click();
          assert(
            await page.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
            `${name} overflow at ${size.width}`,
          );
        }
      }
      await page.setViewportSize({ width: 390, height: 844 });
      await nav("设置").click();
      await page.getByLabel("主题").selectOption("dark");
      await nav("月历").click();
      await page.screenshot({ path: "shots/android/calendar-dark.png" });
      await nav("设置").click();
      await page.getByLabel("主题").selectOption("light");
      await nav("月历").click();
      await page.screenshot({ path: "shots/android/calendar.png" });
      await nav("甘特").click();
      await page.screenshot({ path: "shots/android/gantt.png" });
      await nav("日程").click();
      await page.screenshot({ path: "shots/android/agenda.png" });
      await page.locator(".primary-round").click();
      await dialog().waitFor({ state: 'visible' });
      await page.waitForFunction(() => {
        const layer = document.querySelector('.sheet-layer');
        const sheet = document.querySelector('.sheet');
        return layer && sheet && getComputedStyle(layer).opacity === '1' && getComputedStyle(sheet).transform === 'none';
      });
      await page.screenshot({ path: "shots/android/editor.png", animations: 'disabled' });
      await dialog().getByRole("button", { name: "关闭", exact: true }).click();
    },
  );
  assert.deepEqual(errors, []);
  console.log(
    "Android browser E2E: all checks passed. Screenshots: shots/android",
  );
} finally {
  await context.close();
  await browser.close();
  await new Promise((resolve) => server.httpServer.close(resolve));
}
