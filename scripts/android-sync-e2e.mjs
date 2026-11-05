import { preview } from "vite";
import { chromium } from "playwright-core";
import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";

const server = await preview({ mode: "android", preview: { port: 1438, strictPort: true }, logLevel: "error" });
const browser = await chromium.launch({ channel: "msedge", headless: true });
const key = "CapacitorStorage.fluentcal.mobile.doc.v1";
const today = new Date().toLocaleDateString("sv-SE", { timeZone: "Asia/Shanghai" });
const task = (id, title) => ({ id, title, start: today, end: today, done: false, progress: 0, order: 0, createdAt: 1, updatedAt: 1 });
let cloud = { app: "FluentCal", format: 1, device: "DESKTOP-PC", savedAt: Date.now() - 60_000,
  data: { tasks: { desktop: task("desktop", "电脑上的任务") }, groups: {}, dayMarks: {}, meta: { tasks: { desktop: Date.now() - 60_000 }, groups: {}, dayMarks: {} } } };
let revision = 1, writes = 0, failure = 0, corrupt = false, conflict = false;
const errors = [];
const contexts = [];
async function client() {
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, locale: "zh-CN", timezoneId: "Asia/Shanghai" });
  contexts.push(context);
  await context.addInitScript(({ key }) => {
    if (!localStorage.getItem(key)) localStorage.setItem(key, JSON.stringify({ version: 1, tasks: {}, groups: {}, dayMarks: {}, holidays: {}, settings: { general: { seeded: true, onboarded: true } } }));
  }, { key });
  await context.route("https://dav.example/**", async route => {
    const req = route.request();
    const headers = { "access-control-allow-origin": "*", "access-control-allow-headers": "*", "access-control-expose-headers": "ETag", "content-type": "application/json", etag: `"v${revision}"` };
    if (failure) return route.fulfill({ status: failure, headers, body: "{}" });
    if (req.method() === "OPTIONS") return route.fulfill({ status: 204, headers });
    if (req.method() === "GET") return route.fulfill({ status: 200, headers, body: corrupt ? "{broken" : JSON.stringify(cloud) });
    if (req.method() === "PUT") {
      if (conflict) {
        conflict = false;
        cloud.data.tasks.concurrent = task("concurrent", "电脑同时新建");
        cloud.data.meta.tasks.concurrent = Date.now();
        revision++;
        return route.fulfill({ status: 412, headers, body: "{}" });
      }
      if (req.headers()["if-match"] !== `"v${revision}"`) return route.fulfill({ status: 412, headers, body: "{}" });
      cloud = JSON.parse(req.postData()); revision++; writes++;
      return route.fulfill({ status: 204, headers: { ...headers, etag: `"v${revision}"` } });
    }
    throw new Error("Unexpected WebDAV request " + req.method());
  });
  const page = await context.newPage();
  page.setDefaultTimeout(10_000);
  page.on("pageerror", e => errors.push(e.message));
  await page.goto("http://localhost:1438");
  await page.locator(".bottom-nav").waitFor();
  return { page, context };
}
const nav = (p, name) => p.locator(".bottom-nav").getByRole("button", { name });
const stored = p => p.evaluate(key => JSON.parse(localStorage.getItem(key) ?? "{}"), key);
const waitFor = async (fn, name) => {
  for (let i = 0; i < 150; i++) { if (await fn()) return; await new Promise(r => setTimeout(r, 100)); }
  throw new Error("Timed out: " + name);
};
async function setup(page, mode) {
  await nav(page, "设置").click();
  await page.getByRole("button", { name: "手动配置", exact: true }).click();
  await page.getByLabel("WebDAV 地址", { exact: true }).fill("https://dav.example/dav");
  await page.getByRole("button", { name: "保存并测试连接" }).click();
  await page.getByRole("heading", { name: "发现云端日历" }).waitFor();
  await page.getByRole("button", { name: mode }).click();
  await page.getByRole("button", { name: "立即同步", exact: true }).waitFor();
  await waitFor(async () => (await stored(page)).settings.sync.enabled, "enable persisted");
}
async function create(page, title) {
  await nav(page, "日程").click();
  await page.locator(".primary-round").click();
  await page.getByLabel("任务名称", { exact: true }).fill(title);
  await page.getByRole("dialog").getByRole("button", { name: "保存", exact: true }).click();
  await page.getByRole("dialog").waitFor({ state: "hidden" });
}
async function manual(page) {
  await nav(page, "设置").click();
  await page.getByRole("button", { name: "立即同步", exact: true }).click();
  await page.getByRole("button", { name: "立即同步", exact: true }).isEnabled();
}
const check = async (name, fn) => { await fn(); console.log("PASS " + name); };
try {
  const a = await client(), b = await client();
  await check("merges desktop data, enables second client via cloud replacement and preserves local settings", async () => {
    await create(a.page, "手机初始任务");
    await setup(a.page, "合并并开启同步");
    assert(Object.values((await stored(a.page)).tasks).some(t => t.title === "电脑上的任务"));
    assert(Object.values(cloud.data.tasks).some(t => t.title === "手机初始任务"));
    await create(b.page, "应被替换的本机任务");
    await setup(b.page, "使用云端日历");
    assert(!Object.values((await stored(b.page)).tasks).some(t => t.title === "应被替换的本机任务"));
    assert.equal((await stored(b.page)).settings.theme.mode, "system");
    assert(await b.page.evaluate(() => localStorage.getItem("CapacitorStorage.fluentcal.mobile.before-sync.v1")));
  });
  await check("automatically uploads local edits, merges concurrent cloud writes and pulls on other client", async () => {
    conflict = true;
    await create(a.page, "自动上传测试");
    await waitFor(() => Object.values(cloud.data.tasks).some(t => t.title === "自动上传测试"), "automatic upload");
    assert.equal(cloud.data.tasks.concurrent.title, "电脑同时新建");
    await manual(b.page);
    await waitFor(async () => Object.values((await stored(b.page)).tasks).some(t => t.title === "自动上传测试"), "other client pulls");
  });
  await check("propagates deletions without resurrecting on an offline client", async () => {
    await b.context.setOffline(true);
    await nav(a.page, "日程").click();
    await a.page.locator(".task-main", { hasText: "电脑上的任务" }).click();
    await a.page.getByRole("dialog").getByRole("button", { name: "删除任务", exact: true }).click();
    await a.page.getByRole("dialog").getByRole("button", { name: "确认删除", exact: true }).click();
    await waitFor(() => !cloud.data.tasks.desktop, "delete upload");
    assert(cloud.data.meta.tasks.desktop > 0);
    await b.context.setOffline(false);
    await waitFor(async () => !(await stored(b.page)).tasks.desktop, "online auto pull deletion");
  });
  await check("persists offline edits and reconnects automatically after app restart", async () => {
    await a.context.setOffline(true);
    await create(a.page, "离线修改待上传");
    await new Promise(r => setTimeout(r, 2300));
    assert(!Object.values(cloud.data.tasks).some(t => t.title === "离线修改待上传"));
    await a.context.setOffline(false);
    await a.page.reload();
    await waitFor(() => Object.values(cloud.data.tasks).some(t => t.title === "离线修改待上传"), "restart sync");
    assert((await stored(a.page)).settings.sync.enabled);
  });
  await check("surfaces auth errors, retries manually, and refuses corrupt cloud data without any upload", async () => {
    failure = 401;
    await manual(a.page);
    await a.page.locator(".sync-card .error", { hasText: "认证或权限不足" }).waitFor();
    failure = 0;
    await manual(a.page);
    await waitFor(async () => await a.page.locator(".sync-card .error").count() === 0, "manual auth recovery");
    corrupt = true;
    const before = writes;
    await manual(a.page);
    await a.page.locator(".sync-card .error", { hasText: "没有覆盖云端文件" }).waitFor();
    assert.equal(writes, before);
    corrupt = false;
    await manual(a.page);
    await waitFor(async () => await a.page.locator(".sync-card .error").count() === 0, "remote repair");
  });
  await check("does not commit unchanged data, exports no secrets, and remains usable at 360px", async () => {
    const before = writes;
    await manual(a.page);
    await new Promise(r => setTimeout(r, 500));
    assert.equal(writes, before);
    assert.deepEqual(Object.keys(cloud.data).sort(), ["dayMarks", "groups", "meta", "tasks"]);
    await a.page.setViewportSize({ width: 360, height: 800 });
    await a.page.getByRole("button", { name: "查看配置" }).click();
    assert(await a.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    await mkdir("shots/android", { recursive: true });
    await a.page.locator(".sync-card").screenshot({ path: "shots/android/cloud-sync.png" });
  });
  await check("checks only Android GitHub releases and reports network/asset errors accurately", async () => {
    const release = { tag_name: "android-latest", name: "FluentCal Android v0.4.0", draft: false, prerelease: false, body: "新增手机功能", assets: [
      { name: "FluentCal-Android-0.4.0.apk", state: "uploaded", size: 6000000, browser_download_url: "https://github.com/Lianzy-Baimiao/FluentCal/releases/download/android-latest/FluentCal-Android-0.4.0.apk" } ] };
    let status = 200;
    await a.context.route("https://api.github.com/repos/Lianzy-Baimiao/FluentCal/releases/tags/android-latest", route => route.fulfill({ status, contentType: "application/json", body: JSON.stringify(release) }));
    await a.page.getByRole("button", { name: "检查 GitHub 更新" }).click();
    await a.page.getByText("发现新版本 v0.4.0", { exact: false }).waitFor();
    await a.page.getByRole("button", { name: "下载更新", exact: true }).waitFor();
    status = 429;
    await a.page.getByRole("button", { name: "检查 GitHub 更新" }).click();
    await a.page.getByText("GitHub 暂时限制请求", { exact: false }).waitFor();
    assert.equal(await a.page.getByText("当前已是最新移动端版本", { exact: true }).count(), 0);
  });
  assert.deepEqual(errors, []);
  console.log("Android cloud sync/updates: all 7 integration groups passed (two independent clients).");
} finally {
  for (const c of contexts) await c.close();
  await browser.close();
  await server.httpServer.close();
}
