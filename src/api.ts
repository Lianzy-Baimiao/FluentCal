// Bridge to the Rust backend. In a plain browser (vite dev, screenshot tests) the same calls
// go to an in-memory mock persisted in localStorage and synced between tabs, so every
// window can be developed and checked without Tauri.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import * as web from './webApi';

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const query = new URLSearchParams(typeof location !== 'undefined' ? location.search : '');

export const windowLabel: string = isTauri ? getCurrentWebviewWindow().label : (query.get('w') ?? 'desk');

/** Identifies this window in change events (unique per browser tab in mock mode). */
export const origin: string = isTauri ? windowLabel : `${windowLabel}:${Math.random().toString(36).slice(2, 8)}`;

/** The read-only web view served by FluentCal in the local network (see webApi.ts). */
export const isWeb = !isTauri && windowLabel === 'web';

export function call<T = void>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) return invoke<T>(cmd, args);
  return isWeb ? web.call<T>(cmd, args ?? {}) : mock.call<T>(cmd, args ?? {});
}

/** Subscribes to a backend event; returns an unsubscribe function. */
export function on<T>(event: string, cb: (payload: T) => void): () => void {
  if (isWeb) return web.on(event, cb as (p: unknown) => void);
  if (!isTauri) return mock.on(event, cb as (p: unknown) => void);
  const p = listen<T>(event, (e) => cb(e.payload));
  return () => void p.then((un) => un());
}

export interface AppInfo {
  build: number;
  dark: boolean;
  transparency: boolean;
  accent: string | null;
  accentPalette: string[];
  version: string;
  dataDir: string;
  autostart: boolean;
  shortcutErrors: string[];
  debug: boolean;
}

export interface DataDirInfo {
  path: string;
  custom: boolean;
  /** Configured custom folder that wasn't reachable at start. */
  missing: string | null;
}

export const currentWindow = {
  close: () => (isTauri ? getCurrentWebviewWindow().close() : Promise.resolve(window.close())),
  hide: () => (isTauri ? getCurrentWebviewWindow().hide() : Promise.resolve()),
  show: () => (isTauri ? getCurrentWebviewWindow().show() : Promise.resolve()),
  focus: () => (isTauri ? getCurrentWebviewWindow().setFocus() : Promise.resolve(window.focus())),
  setTheme: (dark: boolean) =>
    isTauri ? getCurrentWebviewWindow().setTheme(dark ? 'dark' : 'light') : Promise.resolve(),
  setTitle: (t: string) => (isTauri ? getCurrentWebviewWindow().setTitle(t) : Promise.resolve(void (document.title = t))),
  startDragging: () => (isTauri ? getCurrentWebviewWindow().startDragging() : Promise.resolve()),
  startResizeDragging: (edge: ResizeEdge) => (isTauri ? getCurrentWebviewWindow().startResizeDragging(edge) : Promise.resolve()),
};

export type ResizeEdge = 'North' | 'South' | 'East' | 'West' | 'NorthEast' | 'NorthWest' | 'SouthEast' | 'SouthWest';

/** A display, as listed by screens_list (see src-tauri/src/screens.rs). */
export interface Screen {
  index: number;
  /** Number as in Windows display settings. */
  number: number;
  width: number;
  height: number;
  scale: number;
  primary: boolean;
  /** The calendar is on this screen. */
  current: boolean;
}

/** Cloud sync status (src-tauri/src/cloud/service.rs Status). */
export interface SyncStatus {
  enabled: boolean;
  provider: string;
  syncing: boolean;
  lastSync: number | null;
  error: string | null;
  errorKind: 'network' | 'auth' | 'config' | 'remote' | 'server' | 'conflict' | null;
  /** Automatic sync stopped until settings change or "sync now". */
  halted: boolean;
  /** Local edits not uploaded yet. */
  pending: boolean;
  remoteDevice: string | null;
  remoteSavedAt: number | null;
}

/** What "test connection" found in the cloud. */
export interface SyncProbe {
  exists: boolean;
  tasks: number;
  groups: number;
  device: string | null;
  savedAt: number | null;
}

export const screenTitle = (s: Screen) => `显示器 ${s.number}${s.primary ? '（主）' : ''}  ${s.width}×${s.height}`;

// ---------------------------------------------------------------------------------------
// Browser mock

type Json = Record<string, unknown>;

function deepMerge(target: Json, patch: Json) {
  for (const [k, v] of Object.entries(patch)) {
    if (v === null) delete target[k];
    else if (typeof v === 'object' && !Array.isArray(v) && typeof target[k] === 'object' && target[k] && !Array.isArray(target[k]))
      deepMerge(target[k] as Json, v as Json);
    else target[k] = v;
  }
}

const mock = (() => {
  const KEY = 'fluentcal.mock.doc';
  const listeners = new Map<string, Set<(p: unknown) => void>>();
  const chan = typeof BroadcastChannel !== 'undefined' ? new BroadcastChannel('fluentcal') : null;
  const emitLocal = (event: string, payload: unknown) => listeners.get(event)?.forEach((cb) => cb(payload));
  if (chan) chan.onmessage = (e) => emitLocal(e.data.event, e.data.payload);
  const broadcast = (event: string, payload: unknown) => {
    emitLocal(event, payload);
    chan?.postMessage({ event, payload });
  };
  const empty = () => ({ version: 1, tasks: {}, groups: {}, dayMarks: {}, holidays: {}, settings: {} });
  const load = (): Json => {
    try {
      const v = JSON.parse(localStorage.getItem(KEY) ?? 'null');
      if (v && typeof v === 'object') return { ...empty(), ...v };
    } catch {
      /* ignore */
    }
    return empty();
  };
  const save = (d: Json) => localStorage.setItem(KEY, JSON.stringify(d));
  const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v));
  let pendingTaken = false;
  let mockToken = '0123456789abcdef0123456789abcdef';
  // Cloud sync stand-in: ?cloud=data pretends the cloud already holds tasks.
  const secrets = new Set<string>();
  const syncStatus: SyncStatus = { enabled: false, provider: '', syncing: false, lastSync: null, error: null, errorKind: null, halted: false, pending: false, remoteDevice: null, remoteSavedAt: null };
  const PROVIDER_NAMES: Record<string, string> = { webdav: 'WebDAV', github: 'GitHub', gitee: 'Gitee', gdrive: 'Google 云端硬盘' };
  let seeded = false;
  /** A preview opened with sync already on (screenshots) starts as "synced a moment ago". */
  const seedSync = (d: Json) => {
    if (seeded) return;
    seeded = true;
    const s = (d.settings as { sync?: { enabled?: boolean; provider?: string } }).sync;
    if (!s?.enabled || !PROVIDER_NAMES[s.provider ?? '']) return;
    Object.assign(syncStatus, { enabled: true, provider: PROVIDER_NAMES[s.provider!], lastSync: Date.now() - 120_000, remoteDevice: 'OFFICE-PC', remoteSavedAt: Date.now() - 3_600_000 });
    const keys: Record<string, string[]> = { webdav: ['webdav'], github: ['github'], gitee: ['gitee'], gdrive: ['google-secret', 'google-refresh'] };
    for (const k of keys[s.provider!] ?? []) secrets.add(k);
  };
  const setSyncEnabled = (d: Json, on: boolean) => {
    deepMerge(d.settings as Json, { sync: { enabled: on } });
    save(d);
    broadcast('store://settings', { origin: 'rust', settings: d.settings });
    const provider = String((d.settings as { sync?: { provider?: string } }).sync?.provider ?? '');
    Object.assign(syncStatus, { enabled: on, provider: PROVIDER_NAMES[provider] ?? '', lastSync: on ? Date.now() : syncStatus.lastSync, error: null });
    broadcast('sync://status', { ...syncStatus });
  };

  async function call<T>(cmd: string, a: Json): Promise<T> {
    const d = load();
    let res: unknown;
    switch (cmd) {
      case 'store_get':
        res = { doc: d, recovered: null };
        break;
      case 'store_put': {
        const col = (d[a.collection as string] ??= {}) as Json;
        const items = clone(a.items as Json);
        for (const [id, v] of Object.entries(items)) {
          if (v === null) delete col[id];
          else col[id] = v;
        }
        save(d);
        broadcast('store://changed', { origin, collection: a.collection, items });
        break;
      }
      case 'store_patch_settings':
        deepMerge(d.settings as Json, clone(a.patch as Json));
        save(d);
        broadcast('store://settings', { origin, settings: d.settings });
        break;
      case 'store_replace':
        save(clone(a.doc as Json));
        broadcast('store://reload', { origin });
        break;
      case 'backups_list':
        res = [];
        break;
      case 'backup_create':
        res = 'manual-browser.json';
        break;
      case 'backup_restore':
        throw new Error('浏览器预览不支持从备份恢复');
      case 'pending_take':
        if (pendingTaken) return null as T;
        pendingTaken = true;
        res = query.get('payload') ? JSON.parse(query.get('payload')!) : query.get('page') ? { page: query.get('page') } : null;
        break;
      case 'settings_open':
        window.open(`?w=settings${a.page ? `&page=${a.page}` : ''}`, 'fluentcal-settings', 'width=980,height=700');
        break;
      case 'editor_open':
        window.open(`?w=editor&payload=${encodeURIComponent(JSON.stringify(a.payload))}`, 'fluentcal-editor', 'width=560,height=720');
        break;
      case 'app_info': {
        const t = query.get('theme');
        res = {
          build: 26300,
          dark: t ? t === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches,
          transparency: true,
          accent: query.get('accent') ?? '#0078D4',
          accentPalette: [],
          version: '0.1.0-web',
          dataDir: '（浏览器预览，数据保存在 localStorage）',
          autostart: false,
          shortcutErrors: [],
          debug: true,
        } satisfies AppInfo;
        break;
      }
      case 'data_dir_info':
        res = { path: '（浏览器预览，数据保存在 localStorage）', custom: false, missing: query.get('missingdir') };
        break;
      case 'screens_list':
        res = [
          { index: 0, number: 1, width: 3840, height: 2160, scale: 1.25, primary: true, current: true },
          { index: 1, number: 2, width: 2560, height: 1600, scale: 1.5, primary: false, current: false },
        ] satisfies Screen[];
        break;
      case 'desk_move_to_screen':
        console.info('[mock] move to screen', a.index);
        break;
      case 'pick_folder':
        res = null;
        break;
      case 'data_dir_set':
        throw new Error('浏览器预览不支持更改数据文件夹');
      case 'notify_test':
        if ('Notification' in window && Notification.permission === 'granted') new Notification('FluentCal 提醒测试');
        else console.info('[mock] notification test');
        break;
      case 'open_url':
        window.open(String(a.url), '_blank', 'noopener');
        break;
      case 'file_save': {
        const bom = a.bom ? '﻿' : '';
        const body = a.bytes ? new Uint8Array(a.bytes as number[]) : bom + String(a.content);
        const url = URL.createObjectURL(new Blob([body], { type: a.bytes ? 'application/octet-stream' : 'text/plain;charset=utf-8' }));
        const link = Object.assign(document.createElement('a'), { href: url, download: String(a.defaultName) });
        link.click();
        setTimeout(() => URL.revokeObjectURL(url), 1000);
        res = String(a.defaultName);
        break;
      }
      case 'file_open':
        res = await new Promise<string | null>((resolve) => {
          const input = Object.assign(document.createElement('input'), {
            type: 'file',
            accept: (a.extensions as string[]).map((e) => `.${e}`).join(','),
          });
          input.onchange = () => {
            const f = input.files?.[0];
            if (!f) return resolve(null);
            f.text().then((t) => resolve(t.replace(/^﻿/, '')));
          };
          input.click();
        });
        break;
      case 'sync_status':
        seedSync(d);
        res = { ...syncStatus };
        break;
      case 'web_info': {
        const w = (d.settings as { web?: { enabled?: boolean; port?: number } }).web ?? {};
        const port = w.port ?? 18765;
        const url = (ip: string) => `http://${ip}:${port}/?w=web#k=${mockToken}`;
        res = { enabled: !!w.enabled, running: !!w.enabled, port, error: null, addresses: [{ ip: '192.168.1.23', label: 'WLAN', virt: false, url: url('192.168.1.23') }], localUrl: url('127.0.0.1') };
        break;
      }
      case 'web_reset_token':
        mockToken = Math.random().toString(16).slice(2).padEnd(32, '0');
        break;
      case 'sync_now':
        if (!syncStatus.enabled) throw new Error('同步没有开启');
        Object.assign(syncStatus, { lastSync: Date.now(), remoteDevice: 'BROWSER', remoteSavedAt: Date.now(), error: null, pending: false });
        broadcast('sync://status', { ...syncStatus });
        res = { ...syncStatus };
        break;
      case 'sync_probe':
        await new Promise((r) => setTimeout(r, 300));
        res = query.get('cloud') === 'data'
          ? ({ exists: true, tasks: 12, groups: 2, device: 'OFFICE-PC', savedAt: Date.now() - 3_600_000 } satisfies SyncProbe)
          : ({ exists: false, tasks: 0, groups: 0, device: null, savedAt: null } satisfies SyncProbe);
        break;
      case 'sync_enable':
        setSyncEnabled(d, true);
        res = { ...syncStatus };
        break;
      case 'sync_disable':
        setSyncEnabled(d, false);
        break;
      case 'sync_set_secret':
        if (a.value) secrets.add(String(a.key));
        else secrets.delete(String(a.key));
        break;
      case 'sync_secrets':
        res = [...secrets];
        break;
      case 'sync_google_login':
        secrets.add('google-refresh');
        deepMerge(d.settings as Json, { sync: { gdrive: { email: 'me@example.com' } } });
        save(d);
        broadcast('store://settings', { origin: 'rust', settings: d.settings });
        res = 'me@example.com';
        break;
      case 'sync_google_logout':
        secrets.delete('google-refresh');
        deepMerge(d.settings as Json, { sync: { gdrive: { email: '' } } });
        save(d);
        broadcast('store://settings', { origin: 'rust', settings: d.settings });
        break;
      default:
        // desk_ready, desk_summon, desk_hide, reminders_set, app_quit, open_data_dir…
        if (cmd === 'reminders_set') console.debug('[mock] reminders', a.items);
    }
    return res as T;
  }

  function on(event: string, cb: (p: unknown) => void): () => void {
    if (!listeners.has(event)) listeners.set(event, new Set());
    listeners.get(event)!.add(cb);
    return () => listeners.get(event)?.delete(cb);
  }

  return { call, on };
})();
