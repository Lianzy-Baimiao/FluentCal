import { beforeAll, describe, expect, it } from 'vitest';

// doc.ts talks to the browser mock in api.ts; give it an in-memory localStorage.
const mem = new Map<string, string>();
(globalThis as Record<string, unknown>).localStorage = {
  getItem: (k: string) => mem.get(k) ?? null,
  setItem: (k: string, v: string) => void mem.set(k, v),
  removeItem: (k: string) => void mem.delete(k),
};

let m: typeof import('./doc');
beforeAll(async () => {
  m = await import('./doc');
  await m.initStore();
});

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const task = (id: string, title: string, start = '2026-10-01') => ({ id, title, start, end: start, progress: 0, done: false, order: 1, createdAt: 0, updatedAt: 0 });

describe('undo / redo', () => {
  it('undoes and redoes a batch of writes', async () => {
    await m.put('tasks', { a: task('a', '写周报') });
    await wait(450);
    await m.put('tasks', { a: task('a', '写周报', '2026-10-02') });
    await m.put('tasks', { b: task('b', '同一批') });
    await wait(450);
    expect((await m.undo()).done).toBe(true); // reverts the move and the new task together
    expect(m.doc.tasks.a.start).toBe('2026-10-01');
    expect(m.doc.tasks.b).toBeUndefined();
    await m.redo();
    expect(m.doc.tasks.a.start).toBe('2026-10-02');
    expect(m.doc.tasks.b.title).toBe('同一批');
  });

  it('leaves items alone that another window changed since', async () => {
    await m.put('tasks', { c: task('c', '原标题') });
    await wait(450);
    await m.put('tasks', { c: task('c', '原标题', '2026-10-05') });
    await wait(450);
    // Simulates store://changed from the editor window (applied to doc, not recorded).
    m.doc.tasks.c = { ...m.doc.tasks.c, title: '编辑器改的标题' };
    const r = await m.undo();
    expect(r.skipped).toBe(1);
    expect(m.doc.tasks.c.title).toBe('编辑器改的标题');
    expect(m.doc.tasks.c.start).toBe('2026-10-05');
  });

  it('deletes and restores', async () => {
    await m.put('tasks', { d: task('d', '要删除的') });
    await wait(450);
    await m.put('tasks', { d: null });
    expect(m.doc.tasks.d).toBeUndefined();
    await m.undo();
    expect(m.doc.tasks.d.title).toBe('要删除的');
  });

  it('does not record writes the app made by itself', async () => {
    await m.put('tasks', { e: task('e', '用户加的') });
    await m.put('tasks', { f: task('f', '示例数据') }, { history: false }); // same 400 ms window
    await m.undo();
    expect(m.doc.tasks.e).toBeUndefined();
    expect(m.doc.tasks.f.title).toBe('示例数据');
  });
});
