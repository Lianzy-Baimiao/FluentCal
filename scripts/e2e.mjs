// End-to-end smoke test of the UI in a plain browser (mock backend, see src/api.ts).
// Builds nothing: run `npm run build` first (the npm "e2e" script does both).
import { preview } from 'vite';
import { chromium } from 'playwright-core';

const server = await preview({ preview: { port: 1431, strictPort: true }, logLevel: 'error' });
const base = 'http://localhost:1431';
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const ctx = await browser.newContext({ viewport: { width: 900, height: 620 } });
const desk = await ctx.newPage();
const errors = [];
desk.on('pageerror', (e) => errors.push(e.message));
const results = [];
async function check(name, fn) {
  try {
    await fn();
    results.push(['PASS', name]);
  } catch (e) {
    results.push(['FAIL', `${name}: ${e.message.split('\n')[0]}`]);
  }
}
const assert = (cond, msg) => {
  if (!cond) throw new Error(msg);
};
const stored = (p) => p.evaluate(() => JSON.parse(localStorage.getItem('fluentcal.mock.doc') ?? '{}'));
const today = new Date();
const iso = (d) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
const plus = (n) => iso(new Date(today.getFullYear(), today.getMonth(), today.getDate() + n));

await desk.goto(`${base}/?w=desk&theme=light`);
await desk.waitForSelector('.chip');

await check('first run seeds sample data', async () => {
  const d = await stored(desk);
  assert(Object.keys(d.tasks ?? {}).length >= 6, 'expected sample tasks');
});

// An empty day in the visible month, so new chips are not hidden behind "+N".
const visible = await desk.evaluate(() => [...document.querySelectorAll('.cell:not(.dim)')].map((c) => c.dataset.date));
const busy = new Set(Object.values((await stored(desk)).tasks).flatMap((t) => [t.start, t.end]));
const target = visible.find((d) => !busy.has(d) && d !== plus(-1) && d !== iso(today)) ?? plus(1);
void plus;
await check('double-click quick add creates tasks', async () => {
  const cell = await desk.locator(`.cell[data-date="${target}"]`).boundingBox();
  await desk.mouse.dblclick(cell.x + cell.width / 2, cell.y + cell.height - 6);
  const box = desk.locator('input.quick-add');
  await box.fill('冒烟测试任务');
  await box.press('Enter');
  await box.fill('第二条');
  await box.press('Enter');
  await box.press('Escape');
  const d = await stored(desk);
  const mine = Object.values(d.tasks).filter((t) => t.title === '冒烟测试任务' || t.title === '第二条');
  assert(mine.length === 2 && mine.every((t) => t.start === target), `got ${mine.length}`);
});

await check('checkbox toggles done', async () => {
  const chip = desk.locator('.chip', { hasText: '冒烟测试任务' });
  await chip.locator('.check').click();
  await desk.waitForTimeout(100);
  const t = Object.values((await stored(desk)).tasks).find((x) => x.title === '冒烟测试任务');
  assert(t.done === true, 'not done');
});

await check('drag a chip to another day', async () => {
  const chip = desk.locator('.chip', { hasText: '第二条' });
  const from = await chip.boundingBox();
  const cells = desk.locator('.cell');
  const idx = await desk.evaluate((d) => [...document.querySelectorAll('.cell')].findIndex((c) => c.dataset.date === d), target);
  const to = await cells.nth(idx > 0 ? idx - 1 : idx + 1).boundingBox();
  await desk.mouse.move(from.x + 30, from.y + from.height / 2);
  await desk.mouse.down();
  await desk.mouse.move(from.x + 40, from.y + 4, { steps: 3 });
  await desk.mouse.move(to.x + to.width / 2, to.y + to.height - 10, { steps: 8 });
  await desk.mouse.up();
  await desk.waitForTimeout(150);
  const t = Object.values((await stored(desk)).tasks).find((x) => x.title === '第二条');
  assert(t.start !== target, `still on ${t.start}`);
});

await check('delete via context menu, then undo', async () => {
  await desk.locator('.chip', { hasText: '冒烟测试任务' }).click({ button: 'right' });
  await desk.getByRole('menuitem', { name: /删除/ }).click();
  await desk.waitForTimeout(100);
  assert(!Object.values((await stored(desk)).tasks).some((x) => x.title === '冒烟测试任务'), 'not deleted');
  await desk.getByRole('button', { name: '撤销' }).click();
  await desk.waitForTimeout(150);
  assert(Object.values((await stored(desk)).tasks).some((x) => x.title === '冒烟测试任务'), 'undo failed');
});

await check('countdown bar shows the next holiday', async () => {
  const text = await desk.locator('.countdown').textContent();
  assert(/距.+还有|就是今天/.test(text), text);
});

await check('search jumps and highlights a day', async () => {
  await desk.keyboard.press('Control+f');
  await desk.keyboard.type('视觉设计');
  await desk.keyboard.press('Enter');
  await desk.waitForSelector('.cell.flash', { timeout: 2000 });
});

await check('Gantt: drag a bar moves the task', async () => {
  await desk.getByText('甘特', { exact: true }).first().click();
  await desk.waitForSelector('.g-chart .bar');
  const bar = desk.locator('.g-chart .bar:not(.rep)', { hasText: '视觉设计' });
  const before = Object.values((await stored(desk)).tasks).find((x) => x.title === '视觉设计');
  const b = await bar.boundingBox();
  await desk.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
  await desk.mouse.down();
  await desk.mouse.move(b.x + b.width / 2 + 70, b.y + b.height / 2, { steps: 6 });
  await desk.mouse.up();
  await desk.waitForTimeout(150);
  const after = Object.values((await stored(desk)).tasks).find((x) => x.title === '视觉设计');
  assert(after.start > before.start && after.end > before.end, `${before.start} -> ${after.start}`);
});

await check('Gantt: zooming keeps the visible dates', async () => {
  const centre = () =>
    desk.evaluate(() => {
      const s = document.querySelector('.g-scroll');
      const t = document.querySelector('.g-table');
      const x = s.scrollLeft + (s.clientWidth - (t?.clientWidth ?? 0)) / 2;
      const hc = [...document.querySelectorAll('.tier.bottom .hc')].find((c) => c.offsetLeft <= x && x < c.offsetLeft + c.offsetWidth);
      return hc?.textContent?.trim();
    });
  const before = await desk.locator('.g-label').textContent();
  await desk.getByRole('radio', { name: '周', exact: true }).click();
  await desk.waitForTimeout(300);
  const after = await desk.locator('.g-label').textContent();
  assert(after === before || Math.abs(parseInt(after.slice(5)) - parseInt(before.slice(5))) <= 1, `${before} -> ${after} (${await centre()})`);
  await desk.getByRole('radio', { name: '日', exact: true }).click();
  await desk.waitForTimeout(300);
});

await check('Gantt: add task in a group', async () => {
  const n = Object.keys((await stored(desk)).tasks).length;
  await desk.getByRole('button', { name: /添加任务/ }).first().click();
  await desk.keyboard.type('甘特新任务');
  await desk.keyboard.press('Enter');
  await desk.waitForTimeout(150);
  const d = await stored(desk);
  assert(Object.keys(d.tasks).length === n + 1 && Object.values(d.tasks).some((t) => t.title === '甘特新任务' && t.groupId), 'not created');
});

await check('Gantt: drag from a bar end onto another bar adds a dependency', async () => {
  const tasks = () => stored(desk).then((d) => Object.values(d.tasks));
  const from = (await tasks()).find((x) => x.title === '需求调研');
  const handle = desk.locator('.g-chart .bar', { hasText: '需求调研' }).locator('.link');
  const h = await handle.boundingBox();
  const t = await desk.locator('.g-chart .bar', { hasText: '前端开发' }).boundingBox();
  await desk.mouse.move(h.x + h.width / 2, h.y + h.height / 2);
  await desk.mouse.down();
  await desk.mouse.move(t.x + t.width / 2, t.y + t.height / 2, { steps: 8 });
  await desk.mouse.up();
  await desk.waitForTimeout(150);
  const to = (await tasks()).find((x) => x.title === '前端开发');
  assert(to.deps?.includes(from.id), `deps: ${JSON.stringify(to.deps)}`);
});

await check('Gantt: export PNG', async () => {
  await desk.getByRole('button', { name: '显示选项' }).click();
  await desk.getByRole('menuitem', { name: /导出图片/ }).hover();
  const [dl] = await Promise.all([desk.waitForEvent('download'), desk.getByRole('menuitem', { name: /PNG/ }).click()]);
  const { fileURLToPath } = await import('node:url');
  const { readFileSync } = await import('node:fs');
  const file = fileURLToPath(new URL('../shots/e2e-gantt.png', import.meta.url));
  await dl.saveAs(file);
  const png = readFileSync(file);
  assert(png.length > 2000 && png.subarray(1, 4).toString() === 'PNG', `size ${png.length}`);
});

await check('editor saves and other windows update', async () => {
  const id = Object.values((await stored(desk)).tasks).find((x) => x.title === '视觉设计').id;
  const ed = await ctx.newPage();
  await ed.goto(`${base}/?w=editor&theme=light&payload=${encodeURIComponent(JSON.stringify({ mode: 'edit', taskId: id }))}`);
  const title = ed.locator('.title-box input');
  await title.fill('视觉设计 V2');
  await ed.keyboard.press('Control+Enter');
  await desk.waitForTimeout(300);
  assert((await desk.locator('.g-chart .bar', { hasText: '视觉设计 V2' }).count()) > 0, 'desk not updated');
});

await check('settings change theme in the desk window', async () => {
  const st = await ctx.newPage();
  await st.goto(`${base}/?w=settings&theme=light&page=appearance`);
  await st.getByRole('radio', { name: '深色', exact: true }).click();
  await desk.waitForTimeout(300);
  assert((await desk.evaluate(() => document.documentElement.dataset.theme)) === 'dark', 'theme not applied');
});

for (const [s, n] of results) console.log(`${s} ${n}`);
if (errors.length) console.log('page errors:\n' + errors.join('\n'));
await browser.close();
await server.close();
process.exit(results.some(([s]) => s === 'FAIL') || errors.length ? 1 : 0);
