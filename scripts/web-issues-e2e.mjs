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
  a: task('a', '跨月旧任务', '2026-11-21', { end: '2026-11-24', groupId: 'work', color: '#12345680' }),
  b: task('b', '甲生日', '2026-11-25', { groupId: 'birth', countdown: true }),
  c: task('c', '乙生日', '2026-11-26', { groupId: 'birth' }),
  d: task('d', '另一项已完成', '2026-10-31', { done: true, groupId: 'work' }),
  e: task('e', '单日颜色测试', '2026-11-21', { color: 'red' }),
  f: task('f', '月度复盘', '2026-11-01', { flex: 'month', flexDone: { '2026-11': '2026-11-05' }, groupId: 'work' }),
  g: task('g', '整理旧照片', '2026-11-18', { flex: 'someday', groupId: 'work' }),
  h: task('h', '每月清账', '2026-11-01', { flex: 'month' }),
};
let showCompleted = false;
let desk = { todayBorderColor: '#22884480', todayBorderWidth: 2, todayFillColor: '', bgColor: '#F3F3F380', bgOpacity: 72, cellOpacity: 55, textColor: 'auto', taskTextMode: 'calendar', textShadow: 'on', textOutlineWidth: 2, fontFamily: 'yahei-ui' };
const documentData = () => ({ version: 1, tasks, groups: { work: { id: 'work', name: '工作', color: '#AA443380', order: 1 }, birth: { id: 'birth', name: '生日', color: 'blue', order: 2 } }, dayMarks: { '2026-11-21': { color: '#AA443380' } }, holidays: {}, settings: { general: { language: 'zh-CN', seeded: true, onboarded: true }, calendar: { view: 'month', listRange: 'upcoming', listGroupLabels: true, holidayCountdown: false, countdownMarked: true, countdownGroups: ['birth'], monthRows: 'six' }, desk, tasks: { showCompleted, completedLast: true, autoPostpone: false, defaultColor: '' }, theme: { mode: 'light', reduceMotion: true } } });
await context.addInitScript(seed => {
  const next = sessionStorage.getItem('fluentcal.test.nextDoc');
  if (next) {
    localStorage.setItem('fluentcal.mock.doc', next);
    sessionStorage.removeItem('fluentcal.test.nextDoc');
  } else if (!sessionStorage.getItem('fluentcal.test.seeded')) {
    localStorage.clear();
    localStorage.setItem('fluentcal.mock.doc', JSON.stringify(seed));
    sessionStorage.setItem('fluentcal.test.seeded', '1');
  }
}, documentData());
await context.route('**/api/doc', r => r.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ doc: documentData(), accent: '#0078D4', pc: 'Test' }) }));
const page = await context.newPage();
const stageMockDoc = () => page.evaluate(doc => sessionStorage.setItem('fluentcal.test.nextDoc', JSON.stringify(doc)), documentData());
const errors = [];
page.on('pageerror', e => errors.push(e.message));
page.setDefaultTimeout(7000);
try {
  await page.goto(`http://localhost:1440/?w=web#k=${'0'.repeat(32)}`);
  await page.locator('.chip').first().waitFor();
  const toolbarHeights = await page.evaluate(() => [
    document.querySelector('.language.f-select')?.getBoundingClientRect().height,
    document.querySelector('.views')?.getBoundingClientRect().height,
  ]);
  assert.deepEqual(toolbarHeights, [32, 32], 'language selector and view tabs share a height');
  assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--cell-bg').trim()), 'rgba(253, 253, 253, 0.276)');
  const calendarTextStyles = await page.evaluate(() => ({
    color: getComputedStyle(document.documentElement).getPropertyValue('--calendar-text').trim(),
    font: getComputedStyle(document.documentElement).getPropertyValue('--calendar-font-family').trim(),
    outline: getComputedStyle(document.documentElement).getPropertyValue('--calendar-outline').trim(),
  }));
  assert.equal(calendarTextStyles.color, '#000000');
  assert.match(calendarTextStyles.font, /Microsoft YaHei UI/);
  assert.match(calendarTextStyles.outline, /2\.0px 0\.0px 0 #FFFFFF/);
  const defaultTaskColor = await page.locator('.chip:not(.bar) .title').first().evaluate(el => getComputedStyle(el).color);
  assert.equal(defaultTaskColor, 'rgb(0, 0, 0)', 'default task label follows adaptive calendar text');
  desk = { ...desk, taskTextMode: 'task' };
  await stageMockDoc();
  await page.reload();
  const taskColor = await page.locator('.chip:not(.bar) .title').first().evaluate(el => getComputedStyle(el).color);
  assert.equal(taskColor, 'rgb(196, 43, 28)', 'task mode restores its own color-specific text');
  const todayCell = page.locator('.cell.today');
  await todayCell.waitFor();
  assert(await todayCell.evaluate(el => el.classList.contains('marked')));
  assert.equal(await todayCell.evaluate(el => getComputedStyle(el).boxShadow), 'rgba(34, 136, 68, 0.5) 0px 0px 0px 2px inset');
  assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--today-border').trim()), '#22884480');
  assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--today-fill').trim()), '');
  desk = { ...desk, todayBorderWidth: 3.5, todayFillColor: '#22884433' };
  await stageMockDoc();
  await page.reload();
  await page.locator('.cell.today-fill').waitFor();
  assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--today-fill').trim()), '#22884433');
  assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--today-border-width').trim()), '3.5px');
  const todayLayers = await page.locator('.cell.today-fill.marked').evaluate(el => getComputedStyle(el).backgroundImage);
  assert.match(todayLayers, /rgba\(34, 136, 68, 0\.2\)/, 'today fill uses its configured alpha');
  assert.match(todayLayers, /color\(srgb 0\.666667 0\.266667 0\.2 \/ 0\.150588\)/, 'day mark remains visible above the translucent fill');
  await page.getByRole('radio', { name: '多周', exact: true }).click();
  const transparentChip = page.locator('.chip.bar[aria-label="跨月旧任务"]').first();
  await transparentChip.waitFor();
  assert.match(await transparentChip.evaluate((el) => getComputedStyle(el).backgroundColor), /rgba\(18, 52, 86, 0\.5\)/, 'web task paints its RGBA fill');
  assert.equal(await page.locator('.cell.marked').first().evaluate(el => el.style.getPropertyValue('--mark')), '#AA443380', 'web date mark retains its source colour and alpha');
  showCompleted = true;
  await stageMockDoc();
  await page.reload();
  await page.locator('.chip').first().waitFor();
  await page.getByRole('radio', { name: '甘特', exact: true }).click();
  await page.getByRole('radio', { name: '列表', exact: true }).click();
  await page.locator('.agenda').waitFor();
  assert.equal(await page.locator('.flex-sections').count(), 0, '近期 keeps undated slots in the calendar column only');
  assert(await page.locator('.day.today h2').innerText().then(text => text.startsWith('今天（2026年11月21日）')));
  await page.getByRole('tab', { name: '所有', exact: true }).click();
  await page.locator('[data-flex-section="month"]').waitFor();
  assert.equal(await page.locator('[data-flex-section="month"] [data-task]').count(), 2, 'all lists current-month slots separately');
  assert.equal(await page.locator('[data-flex-section="someday"] [data-task]').count(), 1, 'all lists open no-date tasks separately');
  assert.equal(await page.locator('[data-flex-section="month"] .flex-note').innerText(), '在桌面勾选时按今天记录：2026年11月21日');
  assert.equal(await page.locator('.day [data-task="f"][data-date="2026-11-05"]').count(), 1, 'completed monthly slot is archived on its completion date');
  await page.getByRole('tab', { name: '近期', exact: true }).click();
  assert.equal(await page.locator('.group-badge').first().innerText(), '工作');
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
  assert.equal(await page.locator('.cell.flash').count(), 4, 'October view highlights the located multi-day span');
  await page.screenshot({ path: 'shots/issues19/web-month-locate.png' });
  await page.setViewportSize({ width: 390, height: 760 });
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), 'narrow layout has no page overflow');
  assert.deepEqual(errors, []);
  console.log('PASS web: countdown union, group labels, filtered completed target, list preservation, multi-day highlight and narrow layout');
} finally {
  await browser.close();
  await server.close();
}
