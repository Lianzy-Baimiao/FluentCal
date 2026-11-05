import { Preferences } from "@capacitor/preferences";
import { Capacitor } from "@capacitor/core";
import { Filesystem, Directory, Encoding } from "@capacitor/filesystem";
import { Share } from "@capacitor/share";
import { emptyDocument, readDocument } from "./data";
import type { Collection, Doc } from "@/types";
import { canonical, cloudView, mergeCloud, SYNCED, type CloudView } from "./sync-model";
import { MOBILE_RELEASE, MOBILE_VERSION } from "./version";
import { AppLauncher } from "@capacitor/app-launcher";

const KEY = "fluentcal.mobile.doc.v1";
const BACKUP = "fluentcal.mobile.previous.v1";
const listeners = new Map<string, Set<(payload: unknown) => void>>();
const emit = (event: string, payload: unknown) =>
  listeners.get(event)?.forEach((cb) => cb(payload));
let current: Doc | undefined;
let recovered: string | null = null;
let queue = Promise.resolve();
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));

async function load() {
  if (current) return current;
  const { value } = await Preferences.get({ key: KEY });
  if (value === null) return (current = emptyDocument());
  try {
    current = readDocument(value);
    if (!current.meta) {
      // Upgrade 0.1 mobile tasks without making old records look freshly edited.
      current.meta = { tasks: Object.fromEntries(Object.entries(current.tasks).map(([id, t]) => [id, Number.isSafeInteger(t.updatedAt) && t.updatedAt > 0 ? t.updatedAt : 0])) };
    }
    return current;
  } catch {
    const previous = await Preferences.get({ key: BACKUP });
    if (!previous.value)
      throw new Error("本机数据损坏且没有可用备份，原始数据已保留");
    current = readDocument(previous.value);
    recovered = "last-saved.json";
    return current;
  }
}

/** Serialize read-modify-write operations; publish only after durable native storage succeeds. */
function transaction(change: (doc: Doc) => void, committed?: (prev: Doc, next: Doc) => void) {
  const pending = queue.then(async () => {
    const prev = await load();
    const next = clone(prev);
    change(next);
    await Preferences.set({ key: BACKUP, value: JSON.stringify(prev) });
    await Preferences.set({ key: KEY, value: JSON.stringify(next) });
    current = next;
    committed?.(prev, next);
    return clone(next);
  });
  queue = pending.then(() => {}).catch(() => {
    emit("store://reload", { origin: "mobile-recovery" });
  });
  return pending;
}

export async function snapshot(): Promise<Doc> {
  await queue;
  return clone(await load());
}

/** Runs against the latest durable data, never the snapshot from before the HTTP request. */
export async function absorb(view: CloudView, replace: boolean, baseline?: CloudView) {
  const saved = await transaction(doc => {
    if (!replace) mergeCloud(doc, view);
    else {
      const next = cloudView(view);
      for (const col of SYNCED) {
        const items = next[col] as Record<string, unknown>;
        // Preserve local edits performed after the replacement request started.
        for (const id of new Set([...Object.keys(doc[col]), ...Object.keys(doc.meta?.[col] ?? {}), ...Object.keys(baseline?.[col] ?? {})])) {
          if (canonical(doc[col][id]) !== canonical(baseline?.[col][id]) || (doc.meta?.[col]?.[id] ?? 0) !== (baseline?.meta?.[col]?.[id] ?? 0)) {
            if (id in doc[col]) items[id] = clone(doc[col][id]);
            else delete items[id];
            (next.meta![col] ??= {})[id] = Math.max(Date.now(), (view.meta?.[col]?.[id] ?? 0) + 1, doc.meta?.[col]?.[id] ?? 0);
          }
        }
      }
      Object.assign(doc, next);
    }
  }, (prev, next) => {
    for (const col of SYNCED) {
      const items: Record<string, unknown> = {};
      for (const id of new Set([...Object.keys(prev[col]), ...Object.keys(next[col])])) {
        if (canonical(prev[col][id]) !== canonical(next[col][id])) items[id] = next[col][id] ?? null;
      }
      if (Object.keys(items).length) emit("store://changed", { origin: "mobile-cloud", collection: col, items });
    }
  });
  return cloudView(saved);
}

export async function backupBeforeSync() {
  if ((await Preferences.get({ key: "fluentcal.mobile.before-sync.v1" })).value) return;
  await Preferences.set({ key: "fluentcal.mobile.before-sync.v1", value: JSON.stringify(await snapshot()) });
}
export async function readSyncBackup() {
  return (await Preferences.get({ key: "fluentcal.mobile.before-sync.v1" })).value;
}

function stamp(doc: Doc, col: Collection, id: string) {
  const meta = ((doc.meta ??= {})[col] ??= {});
  meta[id] = Math.max(Date.now(), (meta[id] ?? 0) + 1);
}

function patchInto(
  target: Record<string, unknown>,
  patch: Record<string, unknown>,
) {
  for (const [key, value] of Object.entries(patch)) {
    if (["__proto__", "prototype", "constructor"].includes(key)) continue;
    if (value && typeof value === "object" && !Array.isArray(value)) {
      if (!target[key] || typeof target[key] !== "object") target[key] = {};
      patchInto(
        target[key] as Record<string, unknown>,
        value as Record<string, unknown>,
      );
    } else if (value === null) delete target[key];
    else target[key] = value;
  }
}

export async function call<T>(
  command: string,
  args: Record<string, unknown>,
): Promise<T> {
  let result: unknown;
  switch (command) {
    case "store_get":
      await queue;
      result = { doc: clone(await load()), recovered };
      break;
    case "store_put":
      if (
        !["tasks", "groups", "holidays", "dayMarks"].includes(
          String(args.collection),
        )
      )
        throw new Error("无效的数据集合");
      await transaction((doc) => {
        const col = doc[args.collection as Collection] as Record<
          string,
          unknown
        >;
        for (const [id, value] of Object.entries(
          args.items as Record<string, unknown>,
        )) {
          if (["__proto__", "constructor", "prototype"].includes(id))
            throw new Error("无效的数据标识");
          if (value === null) delete col[id];
          else col[id] = value;
          stamp(doc, args.collection as Collection, id);
        }
      }, (_prev, next) => {
        // Reconcile optimistic UI writes that may have overlapped a queued cloud merge.
        const collection = args.collection as Collection;
        const items = Object.fromEntries(Object.keys(args.items as Record<string, unknown>).map(id => [id, next[collection][id] ?? null]));
        emit("store://changed", { origin: "mobile-local", collection, items });
      });
      if (args.collection !== "holidays") emit("mobile://dirty", null);
      break;
    case "store_patch_settings":
      await transaction((doc) =>
        patchInto(
          doc.settings as unknown as Record<string, unknown>,
          args.patch as Record<string, unknown>,
        ),
      (_prev, next) => emit("store://settings", { origin: "mobile-settings", settings: clone(next.settings) }));
      emit("mobile://settings", null);
      break;
    case "store_replace":
      await transaction((doc) => {
        const next = readDocument(JSON.stringify(args.doc));
        next.meta = clone(doc.meta ?? {});
        for (const col of [...SYNCED, "holidays"] as const) {
          for (const id of new Set([...Object.keys(doc[col]), ...Object.keys(next[col])])) {
            if (canonical(doc[col][id]) !== canonical(next[col][id])) stamp(next, col, id);
          }
        }
        // Cloud target and enablement belong to this installation, never an imported file.
        next.settings.sync = clone(doc.settings.sync);
        Object.assign(doc, next);
      });
      emit("store://reload", { origin: "mobile-import" });
      emit("mobile://dirty", null);
      break;
    case "app_info":
      result = {
        build: 0,
        dark: matchMedia("(prefers-color-scheme: dark)").matches,
        transparency: false,
        accent: "#0078D4",
        accentPalette: [],
        version: MOBILE_VERSION + "-android",
        dataDir: "应用本地存储",
        autostart: false,
        shortcutErrors: [],
        debug: false,
      };
      break;
    case "open_url": {
      if (args.url !== MOBILE_RELEASE) throw new Error("不支持的链接");
      if (Capacitor.isNativePlatform()) {
        const result = await AppLauncher.openUrl({ url: MOBILE_RELEASE });
        if (!result.completed) throw new Error("无法打开系统浏览器");
      }
      else window.open(MOBILE_RELEASE, "_blank", "noopener,noreferrer");
      break;
    }
    case "file_save": {
      const name = String(args.defaultName).replace(/[^\w.\-]/g, "_");
      const content = (args.bom ? "\uFEFF" : "") + String(args.content);
      if (Capacitor.isNativePlatform()) {
        const { uri } = await Filesystem.writeFile({
          directory: Directory.Cache,
          path: name,
          data: content,
          encoding: Encoding.UTF8,
        });
        await Share.share({
          title: name,
          files: [uri],
          dialogTitle: "保存或分享 FluentCal 数据",
        });
      } else {
        const url = URL.createObjectURL(
          new Blob([content], { type: "application/octet-stream" }),
        );
        const a = Object.assign(document.createElement("a"), {
          href: url,
          download: name,
        });
        a.click();
        setTimeout(() => URL.revokeObjectURL(url), 1000);
      }
      result = name;
      break;
    }
    default:
      throw new Error(`Android 不支持操作：${command}`);
  }
  return result as T;
}

export function on(event: string, cb: (payload: unknown) => void) {
  if (event === "system://theme") {
    const query = matchMedia("(prefers-color-scheme: dark)");
    const update = () => cb(null);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(cb);
  return () => {
    listeners.get(event)?.delete(cb);
  };
}
