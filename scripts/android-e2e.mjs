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
  await mkdir("shots/android", { recursive: true });
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
