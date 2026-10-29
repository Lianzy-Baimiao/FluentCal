// Screenshots with headless Edge.
// node scripts/shot.mjs [--base http://localhost:1430] [--scale 1] [--dark] name=/path?query[@WxH] ...
// Without @WxH the viewport is 1280×900 and the whole page is captured.
import { chromium } from 'playwright-core';
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const args = process.argv.slice(2);
let base = 'http://localhost:1430';
let scale = 1;
let dark = false;
const jobs = [];
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === '--base') base = args[++i];
  else if (a === '--scale') scale = Number(args[++i]) || 1;
  else if (a === '--dark') dark = true;
  else {
    const eq = a.indexOf('=');
    if (eq < 1) throw new Error(`bad job "${a}", expected name=/path[@WxH]`);
    const name = a.slice(0, eq);
    let url = a.slice(eq + 1);
    let size = null;
    const m = /@(\d+)x(\d+)$/.exec(url);
    if (m) {
      size = { width: +m[1], height: +m[2] };
      url = url.slice(0, m.index);
    }
    jobs.push({ name, url, size });
  }
}
if (!jobs.length) {
  console.error('usage: node scripts/shot.mjs [--base URL] [--scale N] [--dark] name=/path?query[@WxH] ...');
  process.exit(2);
}

const outDir = path.join(root, 'shots');
mkdirSync(outDir, { recursive: true });
const browser = await chromium.launch({ channel: 'msedge', headless: true });
try {
  for (const job of jobs) {
    const page = await browser.newPage({
      viewport: job.size ?? { width: 1280, height: 900 },
      deviceScaleFactor: scale,
      colorScheme: dark ? 'dark' : 'light',
    });
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
    await page.goto(new URL(job.url, base).href, { waitUntil: 'networkidle' });
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(400);
    const file = path.join(outDir, `${job.name}.png`);
    await page.screenshot({ path: file, fullPage: !job.size });
    console.log(file + (errors.length ? `  (page errors: ${errors.join(' | ')})` : ''));
    await page.close();
  }
} finally {
  await browser.close();
}
