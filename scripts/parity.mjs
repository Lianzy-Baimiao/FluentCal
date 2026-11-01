// Visual parity of the native rewrite (M10) with 0.2: renders each scene in the browser build
// (the same demo data and fixed date as the README screenshots) and writes, per scene, into
// native/.parity/:
//   <scene>.ref.png       0.2's rendering
//   <scene>.wall.png      the stand-in wallpaper alone (the native snapshot is drawn on it)
//   <scene>.json          scenario for `cargo run --example snapshot -- <scene>.json <out.png>`
// Run `npm run parity` (builds first), then scripts/parity-native.sh to render and compare.
// `node scripts/parity.mjs <scene>...` renders only those; PARITY_SCALE and PARITY_PORT
// (preview port, default 1434) can be set.
//
// Gallery scenes (?w=gallery) take the page's full height and can run `steps` first: each
// step's target is found in 0.2's page, the action is done there, and the same pointer/key
// events go into the scenario for the native side (so both get the same clicks).
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
const small = { width: 560, height: 430 };
const gallery = { width: 1280, height: 900 };
// Gallery steps: [action, target, extra]. Targets are Playwright selectors on 0.2's page.
const SCENES = [
  { name: 'month-light', url: '?w=desk&theme=light', size: month, settings: { theme: { mode: 'light' } } },
  { name: 'month-dark', url: '?w=desk&theme=dark&accent=%238764B8', accent: '#8764B8', size: month, dark: true, settings: { theme: { mode: 'dark', accent: '#8764B8' }, desk: glass } },
  { name: 'weeks-light', url: '?w=desk&theme=light', size: month, settings: { theme: { mode: 'light' }, calendar: { view: 'weeks' } } },
  { name: 'month-small', url: '?w=desk&theme=light', size: { width: 560, height: 430 }, settings: { theme: { mode: 'light' }, calendar: { showWeekNumbers: true } } },
  { name: 'gantt-light', url: '?w=desk&theme=light', size: { width: 1040, height: 548 }, settings: { theme: { mode: 'light' }, calendar: { view: 'gantt' }, gantt: { groupFilter: ['web', 'app'] } } },
  { name: 'gantt-dark', url: '?w=desk&theme=dark', size: { width: 1040, height: 548 }, dark: true, settings: { theme: { mode: 'dark' }, calendar: { view: 'gantt' }, gantt: { scale: 'week' } } },
  // Desk overlays: 0.2 opens them with its steps; the native snapshot opens the same one
  // (`native.open`) at the anchor element's rectangle measured in 0.2 (`native.anchor`).
  { name: 'desk-daylist', url: '?w=desk&theme=light', size: small, settings: { theme: { mode: 'light' } }, steps: [['click', '.cell[data-date="2026-10-12"] .more']], native: { open: 'dayList', date: '2026-10-12', anchor: '.cell[data-date="2026-10-12"] .more' } },
  { name: 'desk-daylist-dark', url: '?w=desk&theme=dark', size: small, dark: true, settings: { theme: { mode: 'dark' } }, steps: [['click', '.cell[data-date="2026-10-12"] .more']], native: { open: 'dayList', date: '2026-10-12', anchor: '.cell[data-date="2026-10-12"] .more' } },
  { name: 'desk-ym', url: '?w=desk&theme=light', size: month, settings: { theme: { mode: 'light' } }, steps: [['click', '.titlebar .title']], native: { open: 'ym', anchor: '.titlebar .title' } },
  { name: 'desk-search', url: '?w=desk&theme=light', size: month, settings: { theme: { mode: 'light' } }, steps: [['key', '', 'Control+f'], ['type', '', '周']], native: { open: 'search', query: '周' } },
  { name: 'desk-search-empty', url: '?w=desk&theme=dark', size: month, dark: true, settings: { theme: { mode: 'dark' } }, steps: [['key', '', 'Control+f']], native: { open: 'search', query: '' } },
  { name: 'desk-adjust', url: '?w=desk&theme=light', size: small, settings: { theme: { mode: 'light' }, desk: { locked: false } } },
  { name: 'desk-welcome', url: '?w=desk&theme=light', size: month, settings: { theme: { mode: 'light' }, general: { onboarded: false } }, doc: (d) => { Object.values(d.tasks)[0].sample = true; } },
  { name: 'gallery-light', window: 'gallery', url: '?w=gallery&theme=light', size: gallery },
  { name: 'gallery-dark', window: 'gallery', url: '?w=gallery&theme=dark', size: gallery, dark: true },
  { name: 'gallery-menu', window: 'gallery', url: '?w=gallery&theme=light', size: gallery, steps: [['click', 'button:has-text("菜单")']] },
  { name: 'gallery-submenu', window: 'gallery', url: '?w=gallery&theme=light', size: gallery, steps: [['click', 'button:has-text("菜单")'], ['hover', '.f-menu .item:has-text("视图")'], ['wait', 400]] },
  { name: 'gallery-select', window: 'gallery', url: '?w=gallery&theme=light', size: gallery, steps: [['click', '.f-select >> nth=0']] },
  { name: 'gallery-date', window: 'gallery', url: '?w=gallery&theme=light', size: gallery, steps: [['click', '.cal-btn >> nth=0']] },
  { name: 'gallery-time', window: 'gallery', url: '?w=gallery&theme=light', size: gallery, steps: [['click', '.clk-btn >> nth=0']] },
  { name: 'gallery-flyout', window: 'gallery', url: '?w=gallery&theme=light', size: gallery, steps: [['click', 'button:has-text("打开浮出层")']] },
  { name: 'gallery-dialog', window: 'gallery', url: '?w=gallery&theme=light', size: gallery, steps: [['click', 'button:has-text("对话框")']] },
  { name: 'gallery-dialog-dark', window: 'gallery', url: '?w=gallery&theme=dark', size: gallery, dark: true, steps: [['click', 'button:has-text("确认框")']] },
  { name: 'gallery-tip', window: 'gallery', url: '?w=gallery&theme=light', size: gallery, steps: [['hover', 'button:has-text("悬停提示")'], ['wait', 700]] },
  { name: 'gallery-focus', window: 'gallery', url: '?w=gallery&theme=light', size: gallery, steps: [['click', 'input[placeholder="搜索任务"]'], ['type', '', '周报'], ['key', '', 'Tab']] },
];
const only = process.argv.slice(2);
// 125 % is this PC's scaling (both screens); PARITY_SCALE=1 / 1.5 / 2 for others.
const SCALE = Number(process.env.PARITY_SCALE || 1.25);
const PORT = Number(process.env.PARITY_PORT || 1434);

/** Runs the steps in 0.2's page and returns the same input as snapshot events. */
async function runSteps(page, steps = []) {
  const events = [];
  for (const [action, target, extra] of steps) {
    if (action === 'wait') {
      await page.waitForTimeout(target);
      events.push({ type: 'wait', ms: target });
      continue;
    }
    if (action === 'key') {
      await page.keyboard.press(extra);
      events.push({ type: 'key', key: extra });
      continue;
    }
    if (action === 'type') {
      await page.keyboard.type(extra);
      events.push({ type: 'text', text: extra });
      continue;
    }
    const box = await page.locator(target).first().boundingBox();
    if (!box) throw new Error(`step target not found: ${target}`);
    const x = Math.round(box.x + box.width / 2);
    const y = Math.round(box.y + box.height / 2);
    if (action === 'click') {
      await page.mouse.move(x, y);
      await page.mouse.down();
      await page.mouse.up();
      events.push({ type: 'click', x, y });
    } else if (action === 'hover') {
      await page.mouse.move(x, y);
      events.push({ type: 'move', x, y });
    }
    await page.waitForTimeout(400);
    events.push({ type: 'wait', ms: 400 });
  }
  return events;
}

const server = await preview({ preview: { port: PORT, strictPort: true }, logLevel: 'error' });
const browser = await chromium.launch({ channel: 'msedge', headless: true });
try {
  for (const s of SCENES.filter((s) => !only.length || only.includes(s.name))) {
    const isGallery = s.window === 'gallery';
    const ctx = await browser.newContext({ viewport: s.size, deviceScaleFactor: SCALE, timezoneId: 'Asia/Shanghai', locale: 'zh-CN', colorScheme: s.dark ? 'dark' : 'light' });
    await ctx.clock.setFixedTime(NOW);
    const doc = isGallery ? demoDoc({ theme: { mode: s.dark ? 'dark' : 'light' } }) : demoDoc(s.settings);
    s.doc?.(doc);
    await ctx.addInitScript((d) => localStorage.setItem('fluentcal.mock.doc', d), JSON.stringify(doc));
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await page.goto(`http://localhost:${PORT}/${s.url}`, { waitUntil: 'networkidle' });
    await page.evaluate(() => document.fonts.ready);
    let size = s.size;
    if (isGallery) {
      // The whole page in one window: no scrolling on either side.
      const h = await page.evaluate(() => document.documentElement.scrollHeight);
      size = { width: s.size.width, height: h };
      await page.setViewportSize(size);
      await page.waitForTimeout(100);
    }
    await page.waitForTimeout(600);
    // The native opener's anchor, measured before the steps change the page.
    let native;
    if (s.native) {
      const { anchor, ...rest } = s.native;
      const box = anchor ? await page.locator(anchor).first().boundingBox() : null;
      if (anchor && !box) throw new Error(`anchor not found: ${anchor}`);
      native = { ...rest, ...(box ? { anchor: [box.x, box.y, box.width, box.height] } : {}) };
    }
    const events = await runSteps(page, s.steps);
    if (s.steps?.length) await page.waitForTimeout(600);
    const base = path.join(outDir, s.name);
    await page.screenshot({ path: `${base}.ref.png` });
    if (!isGallery) {
      // Flyouts, menus and the tooltip are mounted on <body>, outside #app.
      await page.addStyleTag({ content: 'body > * { visibility: hidden !important; }' });
      await page.waitForTimeout(100);
      await page.screenshot({ path: `${base}.wall.png` });
    }
    const scenario = {
      window: s.window ?? 'desk',
      doc, now: NOW.getTime(), offset: 480, width: size.width, height: size.height, scale: SCALE,
      dark: !!s.dark, systemAccent: s.accent ?? '#0078D4', events, ...(native ? { native } : {}),
      ...(isGallery ? {} : { wallpaper: `${base}.wall.png` }),
    };
    writeFileSync(`${base}.json`, JSON.stringify(scenario, null, 1));
    console.log(`${s.name}${errors.length ? `  page errors: ${errors.join(' | ')}` : ''}`);
    await ctx.close();
  }
} finally {
  await browser.close();
  await server.close();
}
