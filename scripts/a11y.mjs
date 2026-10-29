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
const pages = [
  ['desk', '/?w=desk'],
  ['gantt', '/?w=desk', 'gantt'],
  ['settings-appearance', '/?w=settings&page=appearance'],
  ['settings-general', '/?w=settings&page=general'],
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
