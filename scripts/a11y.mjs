// Accessibility audit (axe-core) of the desk, Gantt, settings and editor views, light and dark.
// Run after `npm run build`. Prints violations grouped by rule.
import { preview } from 'vite';
import { chromium } from 'playwright-core';
import { readFileSync } from 'node:fs';

const axe = readFileSync(new URL('../node_modules/axe-core/axe.min.js', import.meta.url), 'utf8');
const server = await preview({ preview: { port: 1432, strictPort: true }, logLevel: 'error' });
const base = 'http://localhost:1432';
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const ctx = await browser.newContext({ viewport: { width: 980, height: 700 } });
// The web view reads /api/doc from FluentCal; here a fixed document stands in.
const today = new Date().toISOString().slice(0, 10);
const webDoc = {
  version: 1, groups: {}, dayMarks: {}, holidays: {}, settings: { theme: { mode: 'system', accent: 'system' } },
  tasks: { a: { id: 'a', title: '网页查看示例', start: today, end: today, time: '09:00', progress: 0, done: false, order: 1, createdAt: 0, updatedAt: 0 } },
};
await ctx.route('**/api/doc', (r) => r.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ doc: webDoc, accent: '#0078D4', pc: 'PC' }) }));
const pages = [
  ['desk', '/?w=desk'],
  ['gantt', '/?w=desk', 'gantt'],
  ['settings-appearance', '/?w=settings&page=appearance'],
  ['settings-general', '/?w=settings&page=general'],
  ['settings-sync', '/?w=settings&page=sync', 'webdav'],
  ['settings-web', '/?w=settings&page=web', 'web-on'],
  ['web-list', '/?w=web', 'web-list'],
  ['web-month', '/?w=web', 'web-month'],
  ['editor', '/?w=editor'],
];
let total = 0;
for (const theme of ['light', 'dark']) {
  for (const [name, url, action] of pages) {
    const p = await ctx.newPage();
    await p.goto(`${base}${url}&theme=${theme}`);
    await p.waitForTimeout(600);
    if (action === 'gantt') {
      await p.getByText('甘特', { exact: true }).first().click();
      await p.waitForTimeout(400);
    } else if (action === 'webdav') {
      await p.getByRole('combobox', { name: '同步方式' }).click();
      await p.getByRole('option', { name: /WebDAV/ }).click();
      await p.waitForTimeout(400);
    } else if (action === 'web-on') {
      const sw = p.getByRole('switch', { name: '允许局域网内的设备查看' });
      if (!(await sw.isChecked())) await sw.click();
      await p.waitForTimeout(800);
    } else if (action?.startsWith('web-')) {
      await p.evaluate(() => localStorage.setItem('fluentcal.web.token', '0123456789abcdef0123456789abcdef'));
      await p.reload();
      await p.getByText('网页查看示例').first().waitFor({ timeout: 5000 });
      await p.getByRole('radio', { name: action === 'web-list' ? '列表' : '月', exact: true }).click();
      await p.waitForTimeout(400);
    }
    await p.addScriptTag({ content: axe });
    const res = await p.evaluate(async () =>
      // the desk calendar sits on the user's wallpaper; contrast there depends on settings
      (await window.axe.run(document, { resultTypes: ['violations'], rules: { region: { enabled: false } } })).violations.map((v) => ({
        id: v.id,
        impact: v.impact,
        n: v.nodes.length,
        sample: v.nodes.slice(0, 2).map((x) => `${x.target.join(' ')} — ${x.failureSummary?.split('\n')[1]?.trim() ?? ''}`),
      })),
    );
    for (const v of res) {
      total += v.n;
      console.log(`[${theme}/${name}] ${v.impact} ${v.id} x${v.n}\n    ${v.sample.join('\n    ')}`);
    }
    if (!res.length) console.log(`[${theme}/${name}] no violations`);
    await p.close();
  }
}
console.log(`total violating nodes: ${total}`);
await browser.close();
await server.close();
