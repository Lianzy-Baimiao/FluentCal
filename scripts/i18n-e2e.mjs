// Exercise the shipped web viewer with an isolated, read-only API fixture.
// Run after building dist/: node scripts/i18n-e2e.mjs
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { preview } from 'vite';
import { chromium } from 'playwright-core';

const server = await preview({ preview: { host: '127.0.0.1', port: 0 }, logLevel: 'error' });
const base = `http://127.0.0.1:${server.httpServer.address().port}`;
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const out = 'shots/i18n';
await mkdir(out, { recursive: true });
const title = '任务 {1} / Weekly plan';
const notes = '保留原文 🌏';
const doc = {
  version: 1,
  tasks: { a: { id: 'a', title, notes, start: '2026-11-03', end: '2026-11-05', time: '09:00', progress: 35, done: false, order: 0, createdAt: 1, updatedAt: 1 } },
  groups: {}, dayMarks: {}, holidays: {},
  // The server's language must not override this browser's preference.
  settings: { general: { language: 'en-US', seeded: true, onboarded: true }, theme: { mode: 'system' }, holidays: { autoUpdate: false } },
};
const errors = [];

async function open(locale, width, colorScheme = 'light') {
  const ctx = await browser.newContext({ locale, colorScheme, viewport: { width, height: 820 }, reducedMotion: 'reduce' });
  await ctx.route('**/api/doc', route => {
    assert.equal(route.request().method(), 'GET');
    assert.equal(route.request().headers().authorization, 'Bearer 0123456789abcdef');
    return route.fulfill({ json: { doc, accent: '#0078D4', pc: 'Preview PC' } });
  });
  const page = await ctx.newPage();
  page.on('pageerror', error => errors.push(error.message));
  await page.clock.install({ time: new Date('2026-11-03T12:00:00+08:00') });
  await page.goto(`${base}/?w=web#k=0123456789abcdef`);
  await page.locator('.foot').filter({ hasText: 'Preview PC' }).waitFor();
  return { ctx, page };
}
const lang = page => page.locator('.language');
async function select(page, name, tag) {
  await lang(page).click();
  await page.getByRole('option', { name, exact: true }).click();
  await page.locator(`html[lang="${tag}"]`).waitFor({ state: 'attached' });
  await page.getByRole('listbox').waitFor({ state: 'hidden' });
}

try {
  const { ctx, page } = await open('zh-CN', 980);
  assert.equal(await page.locator('html').getAttribute('lang'), 'zh-CN');
  assert.equal((await lang(page).innerText()).trim(), '跟随系统');
  // A listbox owns its navigation keys; they must not page the calendar behind it.
  await page.getByRole('radio', { name: '月', exact: true }).click();
  await page.getByRole('button', { name: '下个月', exact: true }).click();
  const month = await page.locator('h1').innerText();
  await lang(page).press('Enter');
  const list = page.getByRole('listbox', { name: '显示语言', exact: true });
  await list.press('Home');
  await list.press('End');
  await list.press('Enter');
  await page.locator('html[lang="en-US"]').waitFor({ state: 'attached' });
  assert.equal(await page.locator('h1').innerText(), month.replace('年', '/').replace('月', ''));
  assert.equal(await lang(page).evaluate(el => el === document.activeElement), true);
  await lang(page).press('Home');
  assert.equal(await page.locator('h1').innerText(), '2026/12');
  await lang(page).press('Enter');
  await page.getByRole('listbox').press('Escape');
  await page.getByRole('listbox').waitFor({ state: 'hidden' });
  assert.equal((await lang(page).innerText()).trim(), 'English');

  await page.getByRole('radio', { name: 'Agenda', exact: true }).click();
  await page.getByRole('button').filter({ hasText: title }).click();
  await page.getByText(notes, { exact: true }).waitFor();
  await page.getByRole('button', { name: 'Close', exact: true }).click();
  await select(page, '繁體中文', 'zh-TW');
  await page.reload();
  await page.getByRole('combobox', { name: '顯示語言', exact: true }).waitFor();
  assert.equal((await lang(page).innerText()).trim(), '繁體中文');
  await select(page, '简体中文', 'zh-CN');
  await select(page, '跟随系统', 'zh-CN');
  await page.evaluate(() => {
    Object.defineProperty(navigator, 'language', { configurable: true, value: 'de-DE' });
    window.dispatchEvent(new Event('languagechange'));
  });
  await page.locator('html[lang="en-US"]').waitFor({ state: 'attached' });
  assert.equal((await lang(page).innerText()).trim(), 'System default');
  assert.equal(await page.evaluate(() => localStorage.getItem('fluentcal.web.language')), 'system');
  await select(page, '简体中文', 'zh-CN');
  await page.evaluate(() => window.dispatchEvent(new Event('languagechange')));
  assert.equal(await page.locator('html').getAttribute('lang'), 'zh-CN');
  await ctx.close();
  console.log('PASS language selection, keyboard navigation, persistence, system changes, unchanged task content');

  for (const [locale, expected] of [['zh-TW', 'zh-TW'], ['zh-HK', 'zh-TW'], ['en-GB', 'en-US'], ['ja-JP', 'en-US']]) {
    const { ctx, page } = await open(locale, 980);
    assert.equal(await page.locator('html').getAttribute('lang'), expected);
    await ctx.close();
  }
  console.log('PASS browser locale defaults and unsupported-language fallback');

  for (const width of [980, 390, 320]) {
    for (const theme of ['light', 'dark']) {
      const { ctx, page } = await open('en-US', width, theme);
      await page.getByRole('radio', { name: 'Month', exact: true }).click();
      await lang(page).click();
      const menu = page.getByRole('listbox', { name: 'Display language', exact: true });
      await menu.waitFor();
      await page.clock.runFor(250);
      for (const element of [lang(page), menu, page.getByRole('radiogroup', { name: 'View', exact: true })]) {
        const box = await element.boundingBox();
        assert.ok(box.x >= 0 && box.x + box.width <= width, `Control outside ${width}px viewport: ${JSON.stringify(box)}`);
      }
      await page.screenshot({ path: `${out}/web-${width}-${theme}.png` });
      await menu.press('Escape');
      for (const view of ['Agenda', 'Weeks', 'Gantt']) {
        await page.getByRole('radio', { name: view, exact: true }).click();
        await select(page, '繁體中文', 'zh-TW');
        await select(page, 'English', 'en-US');
      }
      await ctx.close();
    }
  }
  assert.deepEqual(errors, [], 'Browser runtime errors');
  console.log(`PASS all views, light/dark themes and 980/390/320px layouts; screenshots in ${out}`);
} finally {
  await browser.close();
  await new Promise(resolve => server.httpServer.close(resolve));
}
