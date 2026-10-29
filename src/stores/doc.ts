import { computed, reactive, type WritableComputedRef } from 'vue';
import type { Collection, DayMark, Doc, Group, HolidayYear, Settings, Task } from '@/types';
import { DEFAULT_SETTINGS, mergeDefaults } from '@/defaults';
import { call, on, origin } from '@/api';
import { setHolidayOverrides } from '@/lib/lunar';

type Json = Record<string, unknown>;
export type DeepPartial<T> = { [K in keyof T]?: T[K] extends object ? (T[K] extends unknown[] ? T[K] : DeepPartial<T[K]>) : T[K] };

/** The whole data document, shared by all components of a window and synced across windows. */
export const doc = reactive<Doc>({
  version: 1,
  tasks: {},
  groups: {},
  dayMarks: {},
  holidays: {},
  settings: structuredClone(DEFAULT_SETTINGS),
});

export const storeState = reactive({ ready: false, recovered: null as string | null });

const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v));

function deepMergeInto(target: Json, patch: Json) {
  for (const [k, v] of Object.entries(patch)) {
    if (v === null) delete target[k];
    else if (typeof v === 'object' && !Array.isArray(v) && target[k] && typeof target[k] === 'object' && !Array.isArray(target[k]))
      deepMergeInto(target[k] as Json, v as Json);
    else target[k] = clone(v);
  }
}

function applyDoc(d: Partial<Doc>) {
  doc.tasks = (d.tasks ?? {}) as Record<string, Task>;
  doc.groups = (d.groups ?? {}) as Record<string, Group>;
  doc.dayMarks = (d.dayMarks ?? {}) as Record<string, DayMark>;
  doc.holidays = (d.holidays ?? {}) as Record<string, HolidayYear>;
  doc.settings = mergeDefaults(DEFAULT_SETTINGS, d.settings);
  if (pending) deepMergeInto(doc.settings as unknown as Json, pending);
  setHolidayOverrides(doc.holidays);
}

function applyItems(col: Collection, items: Record<string, unknown>) {
  const target = doc[col] as Record<string, unknown>;
  for (const [id, v] of Object.entries(items)) {
    if (v === null) delete target[id];
    else target[id] = v;
  }
  if (col === 'holidays') setHolidayOverrides(doc.holidays);
}

let init: Promise<void> | null = null;

type Changed = { origin: string; collection: Collection; items: Record<string, unknown> };

function applySettingsEvent(s: Settings) {
  doc.settings = mergeDefaults(DEFAULT_SETTINGS, s);
  if (pending) deepMergeInto(doc.settings as unknown as Json, pending);
}

async function reloadDoc() {
  const r = await call<{ doc: Doc }>('store_get');
  applyDoc(r.doc);
  clearHistory(); // the whole document may have changed (import, restore)
}

export function initStore(): Promise<void> {
  return (init ??= (async () => {
    // Listen before reading the snapshot, and replay what arrives in between, so writes
    // from other windows during start-up are not lost.
    let ready = false;
    const queued: Changed[] = [];
    let queuedSettings: Settings | null = null;
    let reloadAfter = false;
    on<Changed>('store://changed', (p) => {
      if (p.origin === origin) return;
      if (ready) applyItems(p.collection, p.items);
      else queued.push(p);
    });
    on<{ origin: string; settings: Settings }>('store://settings', (p) => {
      if (p.origin === origin) return;
      if (ready) applySettingsEvent(p.settings);
      else queuedSettings = p.settings;
    });
    on('store://reload', () => {
      if (ready) void reloadDoc();
      else reloadAfter = true;
    });
    const res = await call<{ doc: Doc; recovered: string | null }>('store_get');
    applyDoc(res.doc);
    for (const p of queued) applyItems(p.collection, p.items);
    if (queuedSettings) applySettingsEvent(queuedSettings);
    ready = true;
    storeState.recovered = res.recovered;
    storeState.ready = true;
    if (reloadAfter) await reloadDoc();
  })());
}

/** Writes items (null = delete) to a collection, locally first, then to the backend.
 * `history: false` is for writes the user did not make (sample data, auto-postpone). */
export function put<C extends Collection>(col: C, items: Record<string, Doc[C][string] | null>, opts: { history?: boolean } = {}): Promise<void> {
  const plain = clone(items) as Record<string, unknown>;
  if (col !== 'holidays' && opts.history !== false) record(col, plain);
  applyItems(col, plain);
  return call('store_put', { collection: col, items: plain });
}

// ---------------------------------------------------------------- undo / redo
// Each step stores, per item, the value before (`prev`) and after (`next`) the write. Writes
// within 400 ms are one step (e.g. deleting a group also rewrites its tasks); a new click or
// key press always starts a new step. History is per window; an item changed elsewhere since
// (current value != `next`) is left alone on replay.

type Entry = { col: Collection; id: string; prev: unknown; next: unknown };
type Step = Entry[];
const undoStack: Step[] = [];
const redoStack: Step[] = [];
let batch: Step | null = null;
let batchTimer: ReturnType<typeof setTimeout> | undefined;
let replaying = false;
export const history = reactive({ canUndo: false, canRedo: false });

function sync() {
  history.canUndo = undoStack.length > 0 || !!batch;
  history.canRedo = redoStack.length > 0;
}

function clearHistory() {
  clearTimeout(batchTimer);
  undoStack.length = redoStack.length = 0;
  batch = null;
  sync();
}

const same = (a: unknown, b: unknown) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);

function closeBatch() {
  clearTimeout(batchTimer);
  if (batch?.length) undoStack.push(batch);
  if (undoStack.length > 50) undoStack.shift();
  batch = null;
}

if (typeof window !== 'undefined') {
  const boundary = () => {
    if (!batch) return;
    closeBatch();
    sync();
  };
  window.addEventListener('pointerdown', boundary, true);
  window.addEventListener('keydown', boundary, true);
}

function record(col: Collection, items: Record<string, unknown>) {
  if (replaying) return;
  const cur = doc[col] as Record<string, unknown>;
  batch ??= [];
  for (const [id, next] of Object.entries(items)) batch.push({ col, id, prev: id in cur ? clone(cur[id]) : null, next });
  redoStack.length = 0;
  clearTimeout(batchTimer);
  batchTimer = setTimeout(() => (closeBatch(), sync()), 400);
  sync();
}

/** Applies `prev` of each entry (newest first). Returns the inverse step and how many items
 * were skipped because they changed after this step was recorded. */
async function replay(step: Step): Promise<{ inverse: Step; skipped: number }> {
  const inverse: Step = [];
  let skipped = 0;
  replaying = true;
  try {
    for (const e of [...step].reverse()) {
      const cur = (doc[e.col] as Record<string, unknown>)[e.id];
      if (!same(cur, e.next)) {
        skipped++;
        continue;
      }
      // Push in processing order: replaying the inverse (reversed again) runs forward.
      inverse.push({ col: e.col, id: e.id, prev: e.next, next: e.prev });
      await put(e.col, { [e.id]: e.prev } as Record<string, Doc[typeof e.col][string] | null>);
    }
  } finally {
    replaying = false;
  }
  return { inverse, skipped };
}

export interface ReplayResult {
  done: boolean;
  /** Items left as they are because another window changed them meanwhile. */
  skipped: number;
}

export async function undo(): Promise<ReplayResult> {
  closeBatch();
  const step = undoStack.pop();
  let skipped = 0;
  if (step) {
    const r = await replay(step);
    skipped = r.skipped;
    if (r.inverse.length) redoStack.push(r.inverse);
  }
  sync();
  return { done: !!step, skipped };
}

export async function redo(): Promise<ReplayResult> {
  closeBatch();
  const step = redoStack.pop();
  let skipped = 0;
  if (step) {
    const r = await replay(step);
    skipped = r.skipped;
    if (r.inverse.length) undoStack.push(r.inverse);
  }
  sync();
  return { done: !!step, skipped };
}

let pending: Json | null = null;
let timer: ReturnType<typeof setTimeout> | undefined;

/** Deep-merges a settings patch. Rapid patches (slider drags) are coalesced into one IPC call. */
export function patchSettings(patch: DeepPartial<Settings>) {
  const p = clone(patch) as Json;
  deepMergeInto(doc.settings as unknown as Json, p);
  pending ??= {};
  deepMergeInto(pending, p);
  timer ??= setTimeout(flushSettings, 24);
}

export function flushSettings(): Promise<void> {
  clearTimeout(timer);
  timer = undefined;
  const p = pending;
  pending = null;
  return p ? call('store_patch_settings', { patch: p }) : Promise.resolve();
}

/** Writable computed bound to a settings path, e.g. useSetting<number>('desk.bgOpacity'). */
export function useSetting<T>(path: string): WritableComputedRef<T> {
  const keys = path.split('.');
  return computed<T>({
    get: () => keys.reduce<unknown>((o, k) => (o as Json | undefined)?.[k], doc.settings) as T,
    set: (v) => patchSettings(keys.reduceRight<unknown>((acc, k) => ({ [k]: acc }), v) as DeepPartial<Settings>),
  });
}

export function replaceDoc(d: Doc): Promise<void> {
  clearHistory();
  applyDoc(d);
  return call('store_replace', { doc: clone(d) });
}

export function exportDoc(): Doc {
  return clone(doc);
}

export const taskList = computed(() => Object.values(doc.tasks));
export const groupList = computed(() => Object.values(doc.groups).sort((a, b) => a.order - b.order));
