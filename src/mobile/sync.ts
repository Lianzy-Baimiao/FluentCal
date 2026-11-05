import { reactive } from "vue";
import { Preferences } from "@capacitor/preferences";
import type { SyncProbe, SyncStatus } from "@/api";
import * as backend from "./backend";
import { canonical, cloudView, decodeCloud, prune, SyncError, syncRound } from "./sync-model";
import { createRemote, type SyncConfig } from "./sync-remote";
import { getSecret, setSecret } from "./vault";

const STATE_KEY = "fluentcal.mobile.sync-state.v1";
const DEVICE_KEY = "fluentcal.mobile.device.v1";
export const syncStatus = reactive<SyncStatus>({ enabled: false, provider: "", syncing: false, lastSync: null,
  error: null, errorKind: null, halted: false, pending: false, remoteDevice: null, remoteSavedAt: null });
let config: SyncConfig;
let device = "Android";
let running: Promise<void> | undefined;
let changing = false;
let started = false;
let failures = 0;
let debounce: ReturnType<typeof setTimeout> | undefined;
let periodic: ReturnType<typeof setTimeout> | undefined;
let retry: ReturnType<typeof setTimeout> | undefined;
const providerName = (s: SyncConfig) => ({ webdav: "WebDAV", github: "GitHub", gitee: "Gitee", folder: "共享文件夹", "": "" })[s.provider];
const fingerprint = (s: SyncConfig) => canonical({ provider: s.provider, target: s.provider === "webdav" ? s.webdav : s.provider === "github" ? s.github : s.gitee });
function clearTimers() { clearTimeout(debounce); clearTimeout(periodic); clearTimeout(retry); }
function clearError() { Object.assign(syncStatus, { error: null, errorKind: null, halted: false }); }
function report(e: unknown) {
  const error = e instanceof SyncError ? e : new SyncError("config", "无法保存同步数据，请检查手机存储空间后重试");
  Object.assign(syncStatus, { error: error.message, errorKind: error.kind, halted: ["auth", "config", "remote"].includes(error.kind) });
}
function schedulePeriodic() {
  clearTimeout(periodic);
  if (started && config?.enabled && config.interval > 0)
    periodic = setTimeout(() => { void syncNow(false); }, Math.max(1, config.interval) * 60_000);
}
function scheduleEdit() {
  clearTimeout(debounce);
  if (started && config?.enabled && !changing && !syncStatus.halted)
    debounce = setTimeout(() => { void syncNow(false); }, config.provider === "webdav" ? 2000 : 60_000);
}
async function persistStatus() {
  await Preferences.set({ key: STATE_KEY, value: JSON.stringify({ fingerprint: fingerprint(config), lastSync: syncStatus.lastSync,
    remoteDevice: syncStatus.remoteDevice, remoteSavedAt: syncStatus.remoteSavedAt }) });
}
async function refreshConfig() {
  const next = (await backend.snapshot()).settings.sync;
  const changed = !config || canonical(config) !== canonical(next);
  if (config && fingerprint(config) !== fingerprint(next))
    Object.assign(syncStatus, { lastSync: null, remoteDevice: null, remoteSavedAt: null });
  config = next;
  Object.assign(syncStatus, { enabled: config.enabled, provider: providerName(config) });
  if (changed) schedulePeriodic();
}
export async function startSync() {
  if (started) return () => {};
  await refreshConfig();
  const id = (await Preferences.get({ key: DEVICE_KEY })).value;
  device = id || "Android-" + crypto.randomUUID().slice(0, 8);
  if (!id) await Preferences.set({ key: DEVICE_KEY, value: device });
  try {
    const saved = JSON.parse((await Preferences.get({ key: STATE_KEY })).value || "{}");
    if (saved.fingerprint === fingerprint(config)) {
      syncStatus.lastSync = saved.lastSync ?? null;
      syncStatus.remoteDevice = saved.remoteDevice ?? null;
      syncStatus.remoteSavedAt = saved.remoteSavedAt ?? null;
    }
  } catch { /* Nonessential status cache; fresh fetch will reconstruct it. */ }
  started = true;
  const unDirty = backend.on("mobile://dirty", () => { syncStatus.pending = true; scheduleEdit(); });
  const unSettings = backend.on("mobile://settings", () => { void refreshConfig().catch(report); });
  const online = () => { void syncNow(false); };
  window.addEventListener("online", online);
  syncStatus.pending = config.enabled;
  void syncNow(false);
  schedulePeriodic();
  return () => { started = false; clearTimers(); unDirty(); unSettings(); window.removeEventListener("online", online); };
}
/** Errors stay in visible status; callers do not create unhandled rejection popups. */
export function syncNow(manual = true): Promise<void> {
  if (running) return running;
  if (!config?.enabled || changing || (!manual && syncStatus.halted)) return Promise.resolve();
  clearTimeout(debounce);
  clearTimeout(retry);
  if (manual) { failures = 0; clearError(); }
  syncStatus.syncing = true;
  running = (async () => {
    // Even offline failures must settle after `running` receives this promise.
    await Promise.resolve();
    try {
      if (!navigator.onLine) throw new SyncError("network", "当前离线，修改已保存在本机，联网后自动同步");
      const remote = createRemote(config, await getSecret(config.provider));
      const result = await syncRound(backend, remote, device);
      syncStatus.pending = canonical(prune(cloudView(await backend.snapshot()))) !== canonical(result.view);
      Object.assign(syncStatus, { lastSync: Date.now(), remoteDevice: result.remoteDevice, remoteSavedAt: result.remoteSavedAt });
      clearError();
      failures = 0;
      await persistStatus();
    } catch (e) {
      report(e);
      if (!syncStatus.halted && started && config.enabled)
        retry = setTimeout(() => { void syncNow(false); }, Math.min(300_000, 10_000 * 2 ** Math.min(failures++, 5)));
    } finally {
      syncStatus.syncing = false;
      running = undefined;
      if (syncStatus.pending && !syncStatus.error) scheduleEdit();
      schedulePeriodic();
    }
  })();
  return running;
}
export function resumeSync() { return syncNow(false); }

/** Configuration changes and first sync are serialized with in-flight network work. */
async function exclusive<T>(action: () => Promise<T>) {
  if (changing) throw new Error("正在处理同步设置，请稍候");
  changing = true;
  clearTimers();
  try { await running; return await action(); }
  finally { changing = false; schedulePeriodic(); if (syncStatus.pending && !syncStatus.error) scheduleEdit(); }
}
export async function saveSyncConfig(next: SyncConfig, secret?: string) {
  return exclusive(async () => {
    if (config?.enabled) throw new Error("请先关闭同步再修改服务或路径");
    createRemote(next, secret ?? await getSecret(next.provider)); // validate before saving
    if (secret !== undefined) await setSecret(next.provider, secret);
    await backend.call("store_patch_settings", { patch: { sync: { ...next, enabled: false, proxy: "" } } });
    await refreshConfig();
    clearError();
  });
}
export async function probeSync(): Promise<SyncProbe> {
  return exclusive(async () => {
    await refreshConfig();
    const remote = createRemote(config, await getSecret(config.provider));
    const file = await remote.fetch();
    const decoded = file ? decodeCloud(file.text) : null;
    return { exists: !!decoded, tasks: Object.keys(decoded?.view.tasks ?? {}).length, groups: Object.keys(decoded?.view.groups ?? {}).length,
      device: decoded?.device ?? null, savedAt: decoded?.savedAt ?? null };
  });
}
export async function enableSync(mode: "merge" | "replace") {
  return exclusive(async () => {
    syncStatus.syncing = true;
    try {
      await refreshConfig();
      await backend.backupBeforeSync();
      const remote = createRemote(config, await getSecret(config.provider));
      const result = await syncRound(backend, remote, device, mode === "replace");
      await backend.call("store_patch_settings", { patch: { sync: { enabled: true } } });
      await refreshConfig();
      Object.assign(syncStatus, { lastSync: Date.now(), remoteDevice: result.remoteDevice, remoteSavedAt: result.remoteSavedAt,
        pending: canonical(prune(cloudView(await backend.snapshot()))) !== canonical(result.view) });
      clearError();
      await persistStatus();
    } catch (e) { report(e); throw new Error(syncStatus.error!); }
    finally { syncStatus.syncing = false; }
  });
}
export async function disableSync() {
  return exclusive(async () => {
    await backend.call("store_patch_settings", { patch: { sync: { enabled: false } } });
    await refreshConfig();
    clearError();
    clearTimers();
  });
}
export async function updateSecret(value: string) {
  return exclusive(async () => { await setSecret(config.provider, value); clearError(); });
}
