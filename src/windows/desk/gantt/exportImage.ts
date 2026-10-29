import type { ISODate } from '@/types';
import { addDays, diffDays, parts } from '@/lib/date';
import { call } from '@/api';
import { today } from '@/lib/date';
import { bars, dw, gs, links, range, rows, summaries, tableW, tiers, xOfDate } from './model';

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!);
const css = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim() || '#888';
const charW = (s: string) => [...s].reduce((n, c) => n + (c.charCodeAt(0) > 0x2e80 ? 12 : 6.6), 0);

/** Renders the Gantt chart (all rows, task date span) as a standalone SVG string. */
export function ganttSVG(): { svg: string; w: number; h: number } {
  const RH = 32;
  const HEAD = 44;
  const pad = gs.value.scale === 'day' ? 3 : 10;
  let lo: ISODate = '9999-12-31';
  let hi: ISODate = '0000-01-01';
  for (const b of bars.value) {
    if (b.start < lo) lo = b.start;
    if (b.end > hi) hi = b.end;
  }
  if (lo > hi) lo = hi = today();
  lo = addDays(lo, -pad) < range.value.start ? range.value.start : addDays(lo, -pad);
  hi = addDays(hi, pad) > range.value.end ? range.value.end : addDays(hi, pad);
  const x0 = xOfDate(lo);
  const cw = (diffDays(lo, hi) + 1) * dw.value;
  const tw = gs.value.showTable ? Math.max(180, tableW.value) : 0;
  const w = tw + cw;
  const h = HEAD + rows.value.length * RH;
  const c = { bg: css('--solid-base'), text: css('--text-primary'), sub: css('--text-secondary'), line: css('--divider'), accent: css('--accent-fill'), group: css('--subtle-fill-pressed') };
  const font = `font-family="Segoe UI Variable Text, Segoe UI, Microsoft YaHei UI, sans-serif"`;
  const o: string[] = [`<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" ${font} font-size="12">`];
  o.push(`<rect width="${w}" height="${h}" fill="${c.bg}"/>`, `<clipPath id="c"><rect x="${tw}" y="0" width="${cw}" height="${h}"/></clipPath>`);
  for (const [id, color] of [['a', c.sub], ['l', css('--critical')]])
    o.push(`<marker id="${id}" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L8,4 L0,8 z" fill="${color}"/></marker>`);
  // table
  rows.value.forEach((r, i) => {
    const y = HEAD + i * RH;
    if (r.kind === 'group') o.push(`<rect x="0" y="${y}" width="${w}" height="${RH}" fill="${c.group}"/>`);
    o.push(`<line x1="0" x2="${w}" y1="${y + RH - 0.5}" y2="${y + RH - 0.5}" stroke="${c.line}"/>`);
    if (!tw || r.kind === 'add') return;
    const label = r.kind === 'group' ? r.group.name : r.task.title;
    const extra = r.kind === 'group' ? `${r.progress}%` : r.task.repeat || r.task.milestone ? '' : `${r.task.done ? 100 : r.task.progress}%`;
    o.push(`<text x="${r.kind === 'group' ? 10 : 24}" y="${y + 20}" fill="${c.text}" font-weight="${r.kind === 'group' ? 600 : 400}">${esc(label)}</text>`);
    if (extra) o.push(`<text x="${tw - 8}" y="${y + 20}" fill="${c.sub}" text-anchor="end">${extra}</text>`);
  });
  if (tw) o.push(`<line x1="${tw}" x2="${tw}" y1="0" y2="${h}" stroke="${c.line}"/>`);
  // header + chart, shifted so the exported span starts at x = tw
  o.push(`<g clip-path="url(#c)"><g transform="translate(${tw - x0},0)">`);
  for (const t of tiers.value.top) {
    if (t.x + t.w <= x0 || t.x >= x0 + cw) continue; // outside the exported span
    o.push(`<text x="${Math.max(t.x, x0) + 6}" y="16" fill="${c.text}" font-weight="600">${esc(t.label)}</text>`, `<line x1="${t.x}" x2="${t.x}" y1="0" y2="${h}" stroke="${c.line}"/>`);
  }
  for (const t of tiers.value.bottom) {
    if (t.x + t.w < x0 || t.x > x0 + cw) continue;
    o.push(`<text x="${t.x + t.w / 2}" y="37" fill="${t.today ? c.accent : c.sub}" text-anchor="middle" font-size="11">${esc(t.label)}</text>`);
  }
  o.push(`<line x1="${x0}" x2="${x0 + cw}" y1="${HEAD - 0.5}" y2="${HEAD - 0.5}" stroke="${c.line}"/>`);
  const tx = xOfDate(today()) + dw.value / 2;
  o.push(`<line x1="${tx}" x2="${tx}" y1="${HEAD}" y2="${h}" stroke="${c.accent}" stroke-width="2"/>`);
  for (const s of summaries.value) o.push(`<rect x="${s.x}" y="${HEAD + s.row * RH + 11}" width="${s.w}" height="10" rx="2" fill="${s.color}" opacity="0.45"/>`, `<rect x="${s.x}" y="${HEAD + s.row * RH + 11}" width="${(s.w * s.progress) / 100}" height="10" rx="2" fill="${s.color}"/>`);
  for (const l of links.value) o.push(`<path d="${l.d}" transform="translate(0,${HEAD})" fill="none" stroke="${l.late ? css('--critical') : c.sub}" stroke-width="1.25" marker-end="url(#${l.late ? 'l' : 'a'})"/>`);
  for (const b of bars.value) {
    const y = HEAD + b.row * RH;
    const op = b.done ? ' opacity="0.55"' : '';
    if (b.task.milestone) {
      const cx = b.x + dw.value / 2;
      o.push(`<rect x="${cx - 7}" y="${y + 9}" width="14" height="14" rx="2" fill="${b.bar}" transform="rotate(45 ${cx} ${y + 16})"${op}/>`);
      o.push(`<text x="${cx + 14}" y="${y + 20}" fill="${b.text}">${esc(b.task.title)}</text>`);
      continue;
    }
    o.push(`<g${op}><rect x="${b.x + 1}" y="${y + 6}" width="${Math.max(4, b.w - 2)}" height="20" rx="4" fill="${b.bar}"/>`);
    if (b.progress > 0) o.push(`<rect x="${b.x + 1}" y="${y + 6}" width="${Math.max(0, ((b.w - 2) * b.progress) / 100)}" height="20" rx="4" fill="#000" opacity="0.22"/>`);
    const inside = charW(b.task.title) + 14 <= b.w;
    o.push(`<text x="${inside ? b.x + 8 : b.x + b.w + 6}" y="${y + 20}" fill="${inside ? b.onBar : b.text}">${esc(b.task.title)}</text></g>`);
  }
  o.push('</g></g></svg>');
  return { svg: o.join(''), w, h };
}

/** Saves the chart as PNG (2× when it fits) or SVG via the save dialog. */
export async function exportGantt(kind: 'png' | 'svg'): Promise<string | null> {
  const { svg, w, h } = ganttSVG();
  const [y, m, d] = parts(today());
  const name = `甘特图-${y}${String(m).padStart(2, '0')}${String(d).padStart(2, '0')}.${kind}`;
  if (kind === 'svg') return call<string | null>('file_save', { defaultName: name, filterName: 'SVG 图片', extensions: ['svg'], content: svg, bom: false });
  const scale = Math.min(2, 16000 / w, 16000 / h);
  const img = new Image();
  img.src = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
  await img.decode();
  const canvas = Object.assign(document.createElement('canvas'), { width: Math.round(w * scale), height: Math.round(h * scale) });
  const ctx = canvas.getContext('2d')!;
  ctx.scale(scale, scale);
  ctx.drawImage(img, 0, 0);
  URL.revokeObjectURL(img.src);
  const blob = await new Promise<Blob>((res, rej) => canvas.toBlob((b) => (b ? res(b) : rej(new Error('PNG 编码失败'))), 'image/png'));
  const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
  return call<string | null>('file_save', { defaultName: name, filterName: 'PNG 图片', extensions: ['png'], content: '', bytes, bom: false });
}
