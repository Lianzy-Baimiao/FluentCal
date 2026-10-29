import { describe, expect, it } from 'vitest';
import { mergeEdit } from './merge';
import { mk } from './testing';

describe('mergeEdit', () => {
  const base = mk({ id: 'm', title: '旧标题', start: '2026-10-01', end: '2026-10-03', notes: '备注' });

  it('keeps changes made elsewhere in fields the user did not touch', () => {
    const draft = { ...base, title: '新标题' };
    const current = { ...base, done: true, progress: 100, start: '2026-10-05', end: '2026-10-07' };
    expect(mergeEdit(base, draft, current)).toMatchObject({ title: '新标题', done: true, progress: 100, start: '2026-10-05', end: '2026-10-07' });
  });

  it('lets the user win on fields both sides changed, and applies cleared fields', () => {
    const draft = { ...base, start: '2026-10-10', end: '2026-10-12', notes: undefined };
    const current = { ...base, start: '2026-10-05', end: '2026-10-07' };
    const r = mergeEdit(base, draft, current);
    expect(r).toMatchObject({ start: '2026-10-10', end: '2026-10-12' });
    expect(JSON.parse(JSON.stringify(r)).notes).toBeUndefined();
  });

  it('never ends before it starts', () => {
    const draft = { ...base, start: '2026-10-20', end: '2026-10-22' };
    const current = { ...base, end: '2026-10-02' };
    const r = mergeEdit(base, { ...draft, end: base.end }, current);
    expect(r.end >= r.start).toBe(true);
  });
});
