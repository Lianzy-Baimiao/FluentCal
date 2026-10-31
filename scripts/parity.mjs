// Visual parity of the native rewrite (M10) with 0.2: renders each scene in the browser build
// (the same demo data and fixed date as the README screenshots) and writes, per scene, into
// native/.parity/:
//   <scene>.ref.png       0.2's rendering
//   <scene>.wall.png      the stand-in wallpaper alone (the native snapshot is drawn on it)
//   <scene>.json          scenario for `cargo run --example snapshot -- <scene>.json <out.png>`
// Run `npm run parity` (builds first), then scripts/parity-native.sh to render and compare.
import { preview } from 'vite';
import { chromium } from 'playwright-core';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { NOW, demoDoc } from './demo-doc.mjs';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const outDir = path.join(root, 'native', '.parity');
mkdirSync(outDir, { recursive: true });

const glass = { bgOpacity: 18, cellOpacity: 38, shadow: false };
const month = { width: 880, height: 640 };
const SCENES = [
  { name: 'month-light', url: '?w=desk&theme=light', size: month, settings: { theme: { mode: 'light' } } },
  { name: 'month-dark', url: '?w=desk&theme=dark&accent=%238764B8', accent: '#8764B8', size: month, dark: true, settings: { theme: { mode: 'dark', accent: '#8764B8' }, desk: glass } },
  { name: 'weeks-light', url: '?w=desk&theme=light', size: month, settings: { theme: { mode: 'light' }, calendar: { view: 'weeks' } } },
  { name: 'month-small', url: '?w=desk&theme=light', size: { width: 560, height: 430 }, settings: { theme: { mode: 'light' }, calendar: { showWeekNumbers: true } } },
];
const only = process.argv.slice(2);
// 125 % is this PC's scaling (both screens); PARITY_SCALE=1 / 1.5 / 2 for others.
const SCALE = Number(process.env.PARITY_SCALE || 1.25);

const server = await preview({ preview: { port: 1434, strictPort: true }, logLevel: 'error' });
const browser = await chromium.launch({ channel: 'msedge', headless: true });
try {
  for (const s of SCENES.filter((s) => !only.length || only.includes(s.name))) {
    const ctx = await browser.newContext({ viewport: s.size, deviceScaleFactor: SCALE, timezoneId: 'Asia/Shanghai', locale: 'zh-CN', colorScheme: s.dark ? 'dark' : 'light' });
    await ctx.clock.setFixedTime(NOW);
    const doc = demoDoc(s.settings);
    await ctx.addInitScript((d) => localStorage.setItem('fluentcal.mock.doc', d), JSON.stringify(doc));
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await page.goto(`http://localhost:1434/${s.url}`, { waitUntil: 'networkidle' });
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(600);
    const base = path.join(outDir, s.name);
    await page.screenshot({ path: `${base}.ref.png` });
    await page.addStyleTag({ content: '#app { visibility: hidden !important; }' });
    await page.waitForTimeout(100);
    await page.screenshot({ path: `${base}.wall.png` });
    const scenario = {
      doc, now: NOW.getTime(), offset: 480, width: s.size.width, height: s.size.height, scale: SCALE,
      systemAccent: s.accent ?? '#0078D4', wallpaper: `${base}.wall.png`,
    };
    writeFileSync(`${base}.json`, JSON.stringify(scenario, null, 1));
    console.log(`${s.name}${errors.length ? `  page errors: ${errors.join(' | ')}` : ''}`);
    await ctx.close();
  }
} finally {
  await browser.close();
  await server.close();
}
