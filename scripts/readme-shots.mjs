// Screenshots for README.md, from the browser build with made-up demo data and a fixed date,
// so they never show real data and come out the same every time. Run `npm run readme-shots`
// (builds first); writes docs/screenshots/*.png.
import { preview } from 'vite';
import { chromium } from 'playwright-core';
import { execFileSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const outDir = path.join(root, 'docs', 'screenshots');
mkdirSync(outDir, { recursive: true });

// "Today" is Wednesday 2026-10-14: October shows the National Day 休/班 days and two solar terms.
const NOW = new Date('2026-10-14T02:30:00Z'); // 10:30 in Asia/Shanghai
const day = (n) => new Date(Date.UTC(2026, 9, 14 + n)).toISOString().slice(0, 10);

function demoDoc(settings) {
  const ts = NOW.getTime();
  const tasks = {};
  let order = 0;
  const add = (id, t) => (tasks[id] = { id, end: t.start, progress: 0, done: false, order: ++order, createdAt: ts, updatedAt: ts, ...t });
  const groups = {
    web: { id: 'web', name: '官网改版', color: 'blue', order: 1 },
    app: { id: 'app', name: '小程序 2.0', color: 'teal', order: 2 },
  };
  add('research', { title: '需求调研', start: day(-5), end: day(-3), groupId: 'web', progress: 100, done: true });
  add('design', { title: '视觉设计', start: day(-2), end: day(2), groupId: 'web', progress: 45, color: 'purple', deps: ['research'] });
  add('dev', { title: '前端开发', start: day(3), end: day(8), groupId: 'web', color: 'blue', deps: ['design'] });
  add('qa', { title: '测试验收', start: day(9), end: day(11), groupId: 'web', color: 'orange', deps: ['dev'] });
  add('launch', { title: '正式上线', start: day(12), groupId: 'web', milestone: true, color: 'red', deps: ['qa'] });
  add('proto', { title: '交互原型', start: day(-4), end: day(-1), groupId: 'app', progress: 100, done: true, color: 'teal' });
  add('api', { title: '接口联调', start: day(0), end: day(5), groupId: 'app', progress: 15, color: 'teal', deps: ['proto'] });
  add('beta', { title: '内测', start: day(6), end: day(9), groupId: 'app', color: 'indigo', deps: ['api'] });
  add('weekly', { title: '交周报', start: day(2), time: '17:00', reminders: [15], repeat: { freq: 'weekly', weekdays: [5] } });
  add('gym', { title: '健身', start: day(-8), color: 'green', repeat: { freq: 'weekly', weekdays: [2, 4] } });
  add('review', { title: '评审会', start: day(1), time: '14:00', color: 'indigo' });
  add('dentist', { title: '牙医复诊', start: day(7), time: '09:30', color: 'pink' });
  add('reading', { title: '读完《设计心理学》', start: day(14), end: day(18), color: 'yellow' });
  add('trip', { title: '国庆出游', start: day(-13), end: day(-10), color: 'green' });
  add('mom', { title: '妈妈生日', start: day(10), color: 'red', countdown: true, repeat: { freq: 'yearly' } });
  add('bill', { title: '交房租', start: day(17), color: 'gray', repeat: { freq: 'monthly' } });
  add('expense', { title: '报销差旅费', start: day(-1), done: true });
  const dayMarks = { [day(10)]: { color: 'red' }, [day(12)]: { color: 'orange' } };
  return {
    version: 1, tasks, groups, dayMarks, holidays: {},
    settings: { general: { seeded: true }, holidays: { autoUpdate: false, lastCheck: ts }, ...settings },
  };
}

const server = await preview({ preview: { port: 1433, strictPort: true }, logLevel: 'error' });
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const shots = [];
try {
  async function shot(name, { url, size, settings, dark = false, before }) {
    const ctx = await browser.newContext({ viewport: size, deviceScaleFactor: 2, timezoneId: 'Asia/Shanghai', locale: 'zh-CN', colorScheme: dark ? 'dark' : 'light' });
    await ctx.clock.setFixedTime(NOW);
    const doc = JSON.stringify(demoDoc(settings));
    await ctx.addInitScript((d) => localStorage.getItem('fluentcal.mock.doc') || localStorage.setItem('fluentcal.mock.doc', d), doc);
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
