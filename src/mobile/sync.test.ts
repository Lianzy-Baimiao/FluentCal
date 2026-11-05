import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { emptyDocument } from "./data";
import { SyncError } from "./sync-model";

const mocks = vi.hoisted(() => ({
  values: new Map<string, string>(),
  listeners: new Map<string, (p: unknown) => void>(),
  round: vi.fn(),
  snapshot: vi.fn(),
  config: {} as ReturnType<typeof emptyDocument>["settings"]["sync"],
}));
vi.mock("@capacitor/preferences", () => ({ Preferences: {
  get: async ({ key }: { key: string }) => ({ value: mocks.values.get(key) ?? null }),
  set: async ({ key, value }: { key: string; value: string }) => { mocks.values.set(key, value); },
} }));
vi.mock("./backend", () => ({ snapshot: mocks.snapshot, on: (event: string, cb: (p: unknown) => void) => {
  mocks.listeners.set(event, cb); return () => mocks.listeners.delete(event);
}, call: async (_cmd: string, args: any) => { Object.assign(mocks.config, args.patch.sync); }, backupBeforeSync: vi.fn() }));
vi.mock("./sync-remote", () => ({ createRemote: () => ({}) }));
vi.mock("./vault", () => ({ getSecret: async () => "secret", setSecret: vi.fn() }));
vi.mock("./sync-model", async importOriginal => ({ ...await importOriginal<typeof import("./sync-model")>(), syncRound: mocks.round }));
let stop: (() => void) | undefined;
let doc: ReturnType<typeof emptyDocument>;
beforeEach(() => {
  vi.resetModules(); vi.useFakeTimers();
  mocks.values.clear(); mocks.listeners.clear(); mocks.round.mockReset();
  doc = emptyDocument();
  doc.settings.sync.enabled = true; doc.settings.sync.provider = "webdav"; doc.settings.sync.interval = 5;
  mocks.config = doc.settings.sync;
  mocks.snapshot.mockImplementation(async () => structuredClone(doc));
  mocks.round.mockImplementation(async () => ({ view: { tasks: {}, groups: {}, dayMarks: {}, meta: { tasks: {}, groups: {}, dayMarks: {} } }, remoteDevice: "PC", remoteSavedAt: 1 }));
  vi.stubGlobal("window", new EventTarget());
  vi.stubGlobal("navigator", { onLine: true });
});
afterEach(() => { stop?.(); stop = undefined; vi.useRealTimers(); vi.unstubAllGlobals(); });
async function begin() {
  const sync = await import("./sync");
  stop = await sync.startSync();
  await sync.syncNow();
  return sync;
}
it("automatically pulls at startup and configured intervals, then cleans up on dispose", async () => {
  await begin();
  expect(mocks.round).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(300_000);
  expect(mocks.round).toHaveBeenCalledTimes(2);
  stop?.(); stop = undefined;
  await vi.advanceTimersByTimeAsync(300_000);
  expect(mocks.round).toHaveBeenCalledTimes(2);
});
it("debounces local edits and serializes simultaneous manual requests", async () => {
  const sync = await begin();
  mocks.listeners.get("mobile://dirty")!(null);
  await vi.advanceTimersByTimeAsync(1500);
  mocks.listeners.get("mobile://dirty")!(null);
  await vi.advanceTimersByTimeAsync(1500);
  expect(mocks.round).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(500);
  expect(mocks.round).toHaveBeenCalledTimes(2);
  await Promise.all([sync.syncNow(), sync.syncNow(), sync.syncNow()]);
  expect(mocks.round).toHaveBeenCalledTimes(3);
});
it("recovers from startup while offline via the online event", async () => {
  vi.stubGlobal("navigator", { onLine: false });
  const sync = await begin();
  expect(sync.syncStatus.errorKind).toBe("network");
  expect(sync.syncStatus.syncing).toBe(false);
  vi.stubGlobal("navigator", { onLine: true });
  window.dispatchEvent(new Event("online"));
  await sync.syncNow(false);
  expect(mocks.round).toHaveBeenCalledTimes(1);
  expect(sync.syncStatus.error).toBeNull();
});
it("retries network errors with backoff, but pauses auth errors until manual retry", async () => {
  mocks.round.mockRejectedValueOnce(new SyncError("network", "offline"));
  const sync = await begin();
  await vi.advanceTimersByTimeAsync(10_000);
  expect(mocks.round).toHaveBeenCalledTimes(2);
  mocks.round.mockRejectedValueOnce(new SyncError("auth", "expired"));
  await sync.syncNow();
  expect(sync.syncStatus.halted).toBe(true);
  await vi.advanceTimersByTimeAsync(600_000);
  window.dispatchEvent(new Event("online"));
  await sync.syncNow(false);
  expect(mocks.round).toHaveBeenCalledTimes(3);
  await sync.syncNow();
  expect(mocks.round).toHaveBeenCalledTimes(4);
  expect(sync.syncStatus.halted).toBe(false);
});
it("stops all automatic work after disable", async () => {
  const sync = await begin();
  await sync.disableSync();
  mocks.listeners.get("mobile://dirty")!(null);
  await vi.advanceTimersByTimeAsync(600_000);
  window.dispatchEvent(new Event("online"));
  await sync.syncNow();
  expect(mocks.round).toHaveBeenCalledTimes(1);
  expect(sync.syncStatus.enabled).toBe(false);
});
it("batches GitHub edits for a minute", async () => {
  doc.settings.sync.provider = "github";
  await begin();
  mocks.listeners.get("mobile://dirty")!(null);
  await vi.advanceTimersByTimeAsync(59_999);
  expect(mocks.round).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(1);
  expect(mocks.round).toHaveBeenCalledTimes(2);
});
