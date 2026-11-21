// Real read-only web UI: group badges, countdown union and search location across views.
import { preview } from 'vite';
import { chromium } from 'playwright-core';
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';

const server = await preview({ preview: { port: 1440, strictPort: true }, logLevel: 'error' });
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const context = await browser.newContext({ viewport: { width: 900, height: 700 }, timezoneId: 'Asia/Shanghai', locale: 'zh-CN' });
await context.clock.setFixedTime(new Date('2026-11-21T04:00:00Z'));
const task = (id, title, start, extra = {}) => ({ id, title, start, end: start, done: false, progress: 0, order: 1, createdAt: 1, updatedAt: 1, ...extra });
const tasks = {
  a: task('a', '跨月旧任务', '2026-10-30', { end: '2026-11-03', done: true, groupId: 'work' }),
  b: task('b', '甲生日', '2026-11-25', { groupId: 'birth', countdown: true }),
  c: task('c', '乙生日', '2026-11-26', { groupId: 'birth' }),
  d: task('d', '另一项已完成', '2026-10-31', { done: true, groupId: 'work' }),
};
const doc = { version: 1, tasks, groups: { work: { id: 'work', name: '工作', color: 'red', order: 1 }, birth: { id: 'birth', name: '生日', color: 'blue', order: 2 } }, dayMarks: {}, holidays: {}, settings: { general: { language: 'zh-CN', seeded: true, onboarded: true }, calendar: { view: 'month', listRange: 'upcoming', listGroupLabels: true, holidayCountdown: false, countdownMarked: true, countdownGroups: ['birth'] }, tasks: { showCompleted: false }, theme: { mode: 'light', reduceMotion: true } } };
await context.route('**/api/doc', r => r.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ doc, accent: '#0078D4', pc: 'Test' }) }));
const page = await context.newPage();
const errors = [];
page.on('pageerror', e => errors.push(e.message));
page.setDefaultTimeout(7000);
try {
  await page.goto(`http://localhost:1440/?w=web#k=${'0'.repeat(32)}`);
  await page.locator('.chip').first().waitFor();
  assert.equal(await page.locator('.countdown .cd').count(), 2, 'marked and selected group union without duplicates');
  await page.getByRole('radio', { name: '列表', exact: true }).click();
  await page.locator('.agenda').waitFor();
  assert.equal(await page.locator('.group-badge').first().innerText(), '生日');
  await page.getByRole('button', { name: '按分组筛选', exact: true }).click();
  await page.getByRole('checkbox', { name: '生日', exact: true }).click();
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await page.getByPlaceholder('搜索任务、备注或分组').fill('跨月旧任务');
  await page.getByRole('option').filter({ hasText: '跨月旧任务' }).click();
  await page.locator('.agenda [data-task="a"].located').waitFor();
  assert.equal(await page.locator('.agenda').count(), 1, 'search stays in list');
  assert.equal(await page.locator('.agenda [data-task="d"]').count(), 0, 'does not reveal another completed task');
  await mkdir('shots/issues19', { recursive: true });
  await page.screenshot({ path: 'shots/issues19/web-list-locate.png' });
  await page.getByRole('button', { name: '退出定位', exact: true }).click();
  assert.equal(await page.locator('.agenda [data-task="a"]').count(), 0);
  await page.getByRole('radio', { name: '月', exact: true }).click();
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await page.getByPlaceholder('搜索任务、备注或分组').fill('跨月旧任务');
  await page.getByRole('option').filter({ hasText: '跨月旧任务' }).click();
  await page.locator('.cell.flash').first().waitFor();
  assert.equal(await page.locator('.cell.flash').count(), 3, 'October view highlights October 30–November 1; offscreen days are not fabricated');
  await page.screenshot({ path: 'shots/issues19/web-month-locate.png' });
  await page.setViewportSize({ width: 390, height: 760 });
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), 'narrow layout has no page overflow');
  assert.deepEqual(errors, []);
  console.log('PASS web: countdown union, group labels, filtered completed target, list preservation, multi-day highlight and narrow layout');
} finally {
  await browser.close();
  await server.close();
}
