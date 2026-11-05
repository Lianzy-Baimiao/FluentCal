import { preview } from "vite";
import { chromium } from "playwright-core";
import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";

const server = await preview({ mode: "android", preview: { port: 1439, strictPort: true }, logLevel: "error" });
const browser = await chromium.launch({ channel: "msedge", headless: true });
const context = await browser.newContext({ viewport: { width: 360, height: 800 }, isMobile: true, hasTouch: true });
const page = await context.newPage();
const key = "CapacitorStorage.fluentcal.mobile.doc.v1";
try {
  await page.goto("http://localhost:1439");
  await page.locator(".bottom-nav button").first().waitFor();
  await page.waitForFunction(key => Boolean(JSON.parse(localStorage.getItem(key) ?? "null")?.settings?.general?.seeded), key);
  const original = await page.evaluate(key => JSON.parse(localStorage.getItem(key)), key);
  await mkdir("shots/android", { recursive: true });
  for (const theme of ["light", "dark"]) {
    for (const width of [320, 390, 768]) {
      await page.setViewportSize({ width, height: 844 });
      for (const count of [0, 1, 12, 100]) {
        await page.evaluate(({ key, original, count, theme }) => {
          const tasks = Object.fromEntries(Array.from({ length: count }, (_, i) => {
            const id = `badge-${i}`;
            return [id, { id, title: `Task ${i + 1}`, start: "2026-11-05", end: "2026-11-05", done: false, progress: 0, order: i, createdAt: 1, updatedAt: 1 }];
          }));
          const settings = { ...original.settings, general: { ...original.settings.general, seeded: true, onboarded: true }, theme: { ...original.settings.theme, mode: theme } };
          localStorage.setItem(key, JSON.stringify({ ...original, tasks, groups: {}, settings }));
        }, { key, original, count, theme });
        await page.reload();
        const button = page.locator(".bottom-nav button").first();
        await button.waitFor();
        const badge = button.locator("em");
        if (count === 0) { assert.equal(await badge.count(), 0); continue; }
        assert.equal(await badge.innerText(), count > 99 ? "99+" : String(count));
        const rect = await badge.boundingBox();
        const bounds = await button.boundingBox();
        const label = await button.locator(":scope > span").last().boundingBox();
        assert(rect.height >= 18 && rect.width >= 18, `badge too small: ${JSON.stringify(rect)}`);
        assert(rect.x >= bounds.x && rect.x + rect.width <= bounds.x + bounds.width, "badge escapes its button");
        assert(rect.y + rect.height <= label.y, "badge overlaps navigation label");
        const style = await badge.evaluate(el => ({ lineHeight: getComputedStyle(el).lineHeight, nowrap: getComputedStyle(el).whiteSpace }));
        assert.notEqual(style.lineHeight, "normal", "badge needs an explicit line height");
        assert.equal(style.nowrap, "nowrap");
        if (width === 390 && count === 100) await page.screenshot({ path: `shots/android/navigation-${theme}.png` });
      }
    }
  }
  console.log("PASS navigation badges: 0/1/12/99+, 320/390/768px, light/dark");
} finally {
  await context.close();
  await browser.close();
  await new Promise(resolve => server.httpServer.close(resolve));
}
