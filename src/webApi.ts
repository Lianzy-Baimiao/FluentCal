// Backend of the web view (?w=web served by FluentCal in the local network, WEB-02/04): reads
// /api/doc with the access code, every 5 s and when the page comes back to the front. Writes
// are refused; display choices (view, Gantt scale…) stay in this browser.
import type { Doc } from '@/types';

type Json = Record<string, unknown>;
export type WebStatus = 'connecting' | 'ok' | 'offline' | 'unauthorized' | 'noToken';

const TOKEN_KEY = 'fluentcal.web.token';
const PREFS_KEY = 'fluentcal.web.prefs';
const listeners = new Map<string, Set<(p: unknown) => void>>();
let doc: Doc | null = null;
let etag = '';
let accent = '#0078D4';
let pc = '';
let status: WebStatus = 'connecting';
let started = false;
let busy = false;
const waiting: (() => void)[] = [];

function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((cb) => cb(payload));
}

function setStatus(s: WebStatus) {
  if (s === status) return;
  status = s;
  emit('web://status', { status, pc });
}

/** The access code arrives after # in the link; it is kept and taken out of the address bar. */
function token(): string {
  const m = /[#&]k=([0-9a-f]{16,64})/i.exec(location.hash);
  if (m) {
    localStorage.setItem(TOKEN_KEY, m[1]);
    history.replaceState(null, '', location.pathname + location.search);
  }
  return localStorage.getItem(TOKEN_KEY) ?? '';
}

const prefs = (): Json => {
  try {
    return JSON.parse(localStorage.getItem(PREFS_KEY) ?? '{}');
  } catch {
    return {};
  }
};

function deepMerge(target: Json, patch: Json): Json {
  for (const [k, v] of Object.entries(patch)) {
    const t = target[k];
    if (v && typeof v === 'object' && !Array.isArray(v) && t && typeof t === 'object' && !Array.isArray(t)) deepMerge(t as Json, v as Json);
    else if (v === null) delete target[k];
    else target[k] = v;
  }
  return target;
}

async function fetchDoc(): Promise<void> {
  const t = token();
  if (!t) return setStatus('noToken');
  if (busy) return;
  busy = true;
  // A request that never ends (a dropped Wi-Fi connection) must not stop the polling.
  const ac = new AbortController();
  const timer = setTimeout(() => ac.abort(), 10_000);
  try {
    const res = await fetch('/api/doc', { headers: { Authorization: `Bearer ${t}`, ...(etag ? { 'If-None-Match': etag } : {}) }, cache: 'no-store', signal: ac.signal });
    if (res.status === 401) return setStatus('unauthorized');
    if (res.status === 304) return setStatus('ok');
    if (!res.ok) return setStatus('offline');
    const v = (await res.json()) as { doc: Doc; accent: string; pc: string };
    const first = !doc;
    ({ doc, accent, pc } = v);
    etag = res.headers.get('ETag') ?? '';
    setStatus('ok');
    emit('web://status', { status, pc });
    if (first) waiting.splice(0).forEach((f) => f());
    else emit('store://reload', { origin: 'web' });
  } catch {
    setStatus('offline');
  } finally {
    clearTimeout(timer);
    busy = false;
  }
}

function start() {
  if (started) return;
  started = true;
  void fetchDoc();
  // A tab in the background doesn't ask; coming back checks at once (below).
  setInterval(() => document.visibilityState === 'visible' && void fetchDoc(), 5000);
  document.addEventListener('visibilitychange', () => document.visibilityState === 'visible' && void fetchDoc());
}

export async function call<T>(cmd: string, a: Json): Promise<T> {
  let res: unknown = null;
  switch (cmd) {
    case 'store_get': {
      start();
      if (!doc) await new Promise<void>((r) => waiting.push(r));
      const view = structuredClone(doc!) as unknown as Json;
      view.settings = deepMerge((view.settings ?? {}) as Json, prefs());
      res = { doc: view, recovered: null };
      break;
    }
    case 'store_patch_settings':
      localStorage.setItem(PREFS_KEY, JSON.stringify(deepMerge(prefs(), a.patch as Json)));
      break;
    case 'store_put':
    case 'store_replace':
      throw new Error('网页中只能查看');
    case 'app_info': {
      // "System" means this phone's setting (?theme= overrides it, for tests and screenshots).
      const t = new URLSearchParams(location.search).get('theme');
      const dark = t ? t === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches;
      res = { build: 22000, dark, transparency: false, accent, accentPalette: [], version: 'web', dataDir: '', autostart: false, shortcutErrors: [], debug: false };
      break;
    }
    case 'sync_status':
      res = { enabled: false, provider: '', syncing: false, lastSync: null, error: null, errorKind: null, halted: false, pending: false, remoteDevice: null, remoteSavedAt: null };
      break;
    case 'web_status':
      res = { status, pc };
      break;
    // Everything else (reminders, desk window, holiday updates…) has no meaning here.
  }
  return res as T;
}

export function on(event: string, cb: (p: unknown) => void): () => void {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(cb);
  if (event === 'system://theme') {
    // Follow the phone's light/dark switch.
    const mq = matchMedia('(prefers-color-scheme: dark)');
    const f = () => cb(null);
    mq.addEventListener('change', f);
    return () => mq.removeEventListener('change', f);
  }
  return () => listeners.get(event)?.delete(cb);
}
