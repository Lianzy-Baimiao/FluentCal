// Screenshots for README.md, from the browser build with made-up demo data and a fixed date,
// so they never show real data and come out the same every time. Run `npm run readme-shots`
// (builds first); writes docs/screenshots/*.png.
import { preview } from 'vite';
import { chromium } from 'playwright-core';
import { execFileSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { NOW, demoDoc } from './demo-doc.mjs';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const outDir = path.join(root, 'docs', 'screenshots');
mkdirSync(outDir, { recursive: true });

const server = await preview({ preview: { port: 1433, strictPort: true }, logLevel: 'error' });
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const shots = [];
try {
  // `web`: the page a phone gets from FluentCal's web view, with /api/doc answered here the
  // way web.rs answers it (only the display settings).
  async function shot(name, { url, size, settings, dark = false, before, web, storage = {} }) {
    const ctx = await browser.newContext({ viewport: size, deviceScaleFactor: 2, timezoneId: 'Asia/Shanghai', locale: 'zh-CN', colorScheme: dark ? 'dark' : 'light', ...(web && { isMobile: true, hasTouch: true }) });
    await ctx.clock.setFixedTime(NOW);
    const doc = JSON.stringify(demoDoc(settings));
    await ctx.addInitScript((d) => localStorage.getItem('fluentcal.mock.doc') || localStorage.setItem('fluentcal.mock.doc', d), doc);
    await ctx.addInitScript((s) => Object.entries(s).forEach(([k, v]) => localStorage.setItem(k, v)), storage);
    if (web) {
      const { tasks, groups, dayMarks, holidays, settings: s } = demoDoc(settings);
      const shown = Object.fromEntries(Object.entries(s).filter(([k]) => ['calendar', 'tasks', 'gantt', 'theme'].includes(k)));
      const body = JSON.stringify({ doc: { version: 1, tasks, groups, dayMarks, holidays, settings: shown }, accent: '#0078D4', pc: 'MY-PC' });
      await ctx.route('**/api/doc', (r) => r.fulfill({ status: 200, contentType: 'application/json', body }));
    }
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await page.goto(`http://localhost:1433/${url}`, { waitUntil: 'networkidle' });
    await page.evaluate(() => document.fonts.ready);
    if (before) await before(page);
    await page.waitForTimeout(600);
    const file = path.join(outDir, `${name}.png`);
    await page.screenshot({ path: file });
    shots.push(file);
    console.log(`${file}${errors.length ? `  page errors: ${errors.join(' | ')}` : ''}`);
    await ctx.close();
  }
  const month = { width: 880, height: 640 };
  await shot('month-light', { url: '?w=desk&theme=light', size: month, settings: { theme: { mode: 'light' } } });
  // Dark, purple accent, "transparent glass" preset: the (stand-in) wallpaper shows through.
  const glass = { bgOpacity: 18, cellOpacity: 38, shadow: false };
  await shot('month-dark', { url: '?w=desk&theme=dark&accent=%238764B8', size: month, dark: true, settings: { theme: { mode: 'dark', accent: '#8764B8' }, desk: glass } });
  const gantt = { view: 'gantt' };
  await shot('gantt', { url: '?w=desk&theme=light', size: { width: 1040, height: 548 }, settings: { theme: { mode: 'light' }, calendar: gantt, gantt: { groupFilter: ['web', 'app'] } } });
  await shot('settings-appearance', { url: '?w=settings&page=appearance&theme=dark', size: { width: 980, height: 548 }, dark: true, settings: { theme: { mode: 'dark' } } });
  const sync = { enabled: true, provider: 'webdav', webdav: { url: 'https://dav.jianguoyun.com/dav/', user: 'me@example.com', folder: 'FluentCal' } };
  await shot('settings-sync', { url: '?w=settings&page=sync&theme=light', size: { width: 980, height: 590 }, settings: { theme: { mode: 'light' }, sync } });
  const phone = { width: 390, height: 760 };
  const webUrl = `?w=web&theme=light#k=${'0'.repeat(32)}`;
  await shot('web-phone-list', { url: webUrl, size: phone, web: true, settings: { theme: { mode: 'light' } } });
  await shot('web-phone-month', { url: webUrl, size: phone, web: true, settings: { theme: { mode: 'light' } }, storage: { 'fluentcal.web.view': 'month' } });
} finally {
  await browser.close();
  await server.close();
}

// Shots with the wallpaper gradient come out at up to 1 MB; a 256-colour palette with dithering
// makes them about a tenth of that with no visible difference. Flat UI shots stay lossless: a
// palette would merge Fluent's near-identical card greys into the page colour. Needs Pillow.
const QUANTIZE = `import os, sys
from PIL import Image
for f in sys.argv[1:]:
    im = Image.open(f)
    if os.path.getsize(f) > 300_000:
        im = im.convert('RGB').quantize(256, method=Image.Quantize.FASTOCTREE, dither=Image.Dither.FLOYDSTEINBERG)
    im.save(f, optimize=True)`;
try {
  execFileSync('python', ['-c', QUANTIZE, ...shots], { stdio: 'inherit' });
} catch {
  console.warn('Python with Pillow not found: screenshots left uncompressed.');
}
