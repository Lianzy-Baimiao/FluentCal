import { describe, expect, it } from 'vitest';
import type { Group } from '@/types';
import { buildRows, canLink, chartRange, dateAt, DAY_WIDTH, depPath, headerTiers, summarize, UNGROUPED, wouldCycle, xOf } from './gantt';
import { mk } from './testing';

const g1: Group = { id: 'g1', name: '项目 A', color: 'blue', order: 2 };
const g2: Group = { id: 'g2', name: '项目 B', color: 'green', order: 1, collapsed: true };
const opts = { hideDone: false, hideSingleDay: true, groupFilter: [] as string[] };

describe('gantt', () => {
  const a = mk({ id: 'a', start: '2026-10-05', end: '2026-10-09', groupId: 'g1', progress: 50 });
  const b = mk({ id: 'b', start: '2026-10-01', end: '2026-10-02', groupId: 'g1', done: true });
  const c = mk({ id: 'c', start: '2026-10-03', groupId: 'g2' });
  const lone = mk({ id: 'lone', start: '2026-10-04' });
  const ms = mk({ id: 'ms', start: '2026-10-20', milestone: true });

  it('builds group, task and add rows', () => {
    const rows = buildRows([a, b, c, lone, ms], [g1, g2], opts);
    expect(rows.map((r) => r.key)).toEqual(['g:g2', 'g:g1', 't:b', 't:a', 'a:g1', `g:${UNGROUPED}`, 't:ms', `a:${UNGROUPED}`]);
    const filtered = buildRows([a, b], [g1, g2], { ...opts, hideDone: true, groupFilter: ['g1'] });
    expect(filtered.map((r) => r.key)).toEqual(['g:g1', 't:a', 'a:g1']);
  });

  it('summarizes a group', () => {
    // a: 5 days at 50%, b: 2 days done
    expect(summarize([a, b])).toEqual({ start: '2026-10-01', end: '2026-10-09', progress: Math.round((5 * 50 + 2 * 100) / 7) });
    expect(summarize([])).toEqual({ progress: 0 });
  });

  it('aligns the chart range to whole months', () => {
    const r = chartRange([a, ms], '2026-10-29', 'day');
    expect(r.start).toBe('2026-09-01');
    expect(r.end.endsWith('-31') || r.end.endsWith('-30')).toBe(true);
    expect(r.end > '2027-01-01').toBe(true);
  });

  it('maps dates to pixels and back', () => {
    const dw = DAY_WIDTH.day;
    expect(xOf('2026-10-11', '2026-10-01', dw)).toBe(10 * dw);
    expect(dateAt(10 * dw + dw - 1, '2026-10-01', dw)).toBe('2026-10-11');
    expect(dateAt(-1, '2026-10-01', dw)).toBe('2026-09-30');
  });

  it('builds header tiers', () => {
    const range = { start: '2026-10-01', end: '2026-11-30' };
    const day = headerTiers(range, 'day', '2026-10-29', 1);
    expect(day.top.map((c) => c.label)).toEqual(['2026年10月', '2026年11月']);
    expect(day.bottom).toHaveLength(61);
    expect(day.bottom.find((c) => c.today)?.date).toBe('2026-10-29');
    const month = headerTiers({ start: '2026-10-01', end: '2027-03-31' }, 'month', '2026-10-29', 1);
    expect(month.top.map((c) => c.label)).toEqual(['2026年', '2027年']);
    expect(month.bottom).toHaveLength(6);
    expect(month.top[0].w + month.top[1].w).toBe(182 * DAY_WIDTH.month);
  });

  it('rejects dependency links that would loop', () => {
    const x = mk({ id: 'x', start: '2026-10-01' });
    const y = mk({ id: 'y', start: '2026-10-02', deps: ['x'] });
    const z = mk({ id: 'z', start: '2026-10-03', deps: ['y'] });
    const r = mk({ id: 'r', start: '2026-10-03', repeat: { freq: 'daily' } });
    const all = { x, y, z, r };
    expect(canLink(all, 'z', 'x')).toBe(false); // x → y → z → x
    expect(canLink(all, 'x', 'z')).toBe(true);
    expect(canLink(all, 'x', 'y')).toBe(false); // already linked
    expect(canLink(all, 'x', 'x')).toBe(false);
    expect(canLink(all, 'x', 'r')).toBe(false); // repeating tasks have no single bar
    expect(wouldCycle(all, 'y', 'x')).toBe(true);
  });

  it('routes dependency arrows', () => {
    expect(depPath(100, 16, 200, 48)).toBe('M100,16 H108 V48 H200');
    expect(depPath(100, 16, 90, 48)).toContain('V32');
  });
});
