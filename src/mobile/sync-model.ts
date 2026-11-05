import type { Doc } from "@/types";
import { readDocument } from "./data";

export const SYNCED = ["tasks", "groups", "dayMarks"] as const;
export const FILE_NAME = "fluentcal-data.json";
export const MAX_BYTES = 20_000_000;
export type CloudView = Pick<Doc, "tasks" | "groups" | "dayMarks" | "meta">;
export type ErrorKind = "network" | "auth" | "config" | "remote" | "server" | "conflict";
export class SyncError extends Error {
  constructor(public kind: ErrorKind, message: string) { super(message); }
}
export const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v));
export function canonical(value: unknown): string {
  if (Array.isArray(value)) return "[" + value.map(canonical).join(",") + "]";
  if (value && typeof value === "object") {
    const v = value as Record<string, unknown>;
    return "{" + Object.keys(v).sort().map(k => JSON.stringify(k) + ":" + canonical(v[k])).join(",") + "}";
  }
  return JSON.stringify(value ?? null);
}
export function cloudView(doc: CloudView): CloudView {
  return clone({ tasks: doc.tasks, groups: doc.groups, dayMarks: doc.dayMarks,
    meta: Object.fromEntries(SYNCED.map(col => [col, doc.meta?.[col] ?? {}])) });
}
export function prune(view: CloudView, now = Date.now()) {
  for (const col of SYNCED) {
    for (const [id, ts] of Object.entries(view.meta?.[col] ?? {})) {
      if (!(id in view[col]) && ts < now - 60 * 86_400_000) delete view.meta![col]![id];
    }
  }
  return view;
}
/** Same last-write-wins and legacy/tie rules as native/core/src/merge.rs. */
export function mergeCloud(ours: CloudView, theirs: CloudView) {
  ours.meta ??= {};
  for (const col of SYNCED) {
    const local = ours[col] as Record<string, unknown>;
    const remote = theirs[col] as Record<string, unknown>;
    const stamps = (ours.meta[col] ??= {});
    const other = theirs.meta?.[col] ?? {};
    for (const id of new Set([...Object.keys(local), ...Object.keys(remote), ...Object.keys(stamps), ...Object.keys(other)])) {
      const a = stamps[id] ?? 0, b = other[id] ?? 0;
      if (b > a || (a === 0 && b === 0 && !(id in local) && id in remote)) {
        if (id in remote) local[id] = clone(remote[id]);
        else delete local[id];
        stamps[id] = b;
      }
    }
  }
  return prune(ours);
}
export function decodeCloud(text: string): { view: CloudView; device: string | null; savedAt: number | null } | null {
  if (new TextEncoder().encode(text).length > MAX_BYTES) throw new SyncError("remote", "云端文件超过 20 MB，已停止同步");
  const clean = text.replace(/^\uFEFF/, "").trim();
  if (!clean) return null;
  try {
    const file = JSON.parse(clean);
    if (file?.app !== "FluentCal" || !Number.isInteger(file.format) || file.format < 1 || file.format > 1)
      throw new Error("云端文件不是受支持的 FluentCal 同步格式，请检查路径或升级客户端");
    for (const col of SYNCED) {
      if (!file.data?.[col] || typeof file.data[col] !== "object" || Array.isArray(file.data[col]))
        throw new Error("云端数据集合不完整");
    }
    // Validate all records before merging, but keep the wire values (including optional
    // and future fields) byte-semantically intact, avoiding needless Git commits.
    readDocument(JSON.stringify({ ...file.data, version: 1, settings: undefined, holidays: {} }));
    return { view: prune(cloudView(file.data)), device: typeof file.device === "string" ? file.device : null,
      savedAt: Number.isSafeInteger(file.savedAt) ? file.savedAt : null };
  } catch (e) {
    throw new SyncError("remote", (e instanceof Error && !(e instanceof SyntaxError) ? e.message : "云端文件已损坏") + "；没有覆盖云端文件");
  }
}
export function encodeCloud(view: CloudView, device: string, savedAt = Date.now()) {
  const text = JSON.stringify({ app: "FluentCal", format: 1, device, savedAt, data: cloudView(view) });
  if (new TextEncoder().encode(text).length > MAX_BYTES) throw new SyncError("config", "同步数据超过 20 MB，请减少历史任务");
  return text;
}

export interface RemoteFile { text: string; revision: string | null }
export interface Remote {
  fetch(): Promise<RemoteFile | null>;
  put(text: string, base: RemoteFile | null): Promise<void>;
}
export interface SyncStore {
  snapshot(): Promise<Doc>;
  absorb(view: CloudView, replace: boolean, baseline?: CloudView): Promise<CloudView>;
}
/** Fetch every round: no cached revisions can become detached from their content. */
export async function syncRound(store: SyncStore, remote: Remote, device: string, replace = false) {
  // For first-time replacement, edits made while the request is in flight still win locally.
  const baseline = replace ? cloudView(await store.snapshot()) : undefined;
  let remoteDevice: string | null = null, remoteSavedAt: number | null = null;
  for (let attempt = 0; attempt < 3; attempt++) {
    const file = await remote.fetch();
    const decoded = file ? decodeCloud(file.text) : null;
    if (replace && !decoded) throw new SyncError("remote", "云端数据已被移走，请重新测试连接后选择合并");
    if (decoded) {
      remoteDevice = decoded.device;
      remoteSavedAt = decoded.savedAt;
      await store.absorb(decoded.view, replace, baseline);
      replace = false;
    }
    const merged = prune(cloudView(await store.snapshot()));
    if (decoded && canonical(merged) === canonical(decoded.view)) return { view: merged, remoteDevice, remoteSavedAt };
    if (!decoded && SYNCED.every(c => !Object.keys(merged[c]).length && !Object.keys(merged.meta?.[c] ?? {}).length))
      return { view: merged, remoteDevice, remoteSavedAt };
    const at = Date.now();
    try {
      await remote.put(encodeCloud(merged, device, at), file);
      return { view: merged, remoteDevice: device, remoteSavedAt: at };
    } catch (e) {
      if (!(e instanceof SyncError) || e.kind !== "conflict") throw e;
    }
  }
  throw new SyncError("server", "云端正被其他设备修改，稍后自动重试");
}
