import { beforeEach, expect, it, vi } from "vitest";
import { emptyDocument } from "./data";
const storage = vi.hoisted(() => ({
  values: new Map<string, string>(),
  fail: false,
}));
vi.mock("@capacitor/preferences", () => ({
  Preferences: {
    get: async ({ key }: { key: string }) => ({
      value: storage.values.get(key) ?? null,
    }),
    set: async ({ key, value }: { key: string; value: string }) => {
      await new Promise((r) => setTimeout(r, 2));
      if (storage.fail && key === "fluentcal.mobile.doc.v1")
        throw new Error("disk full");
      storage.values.set(key, value);
    },
  },
}));
beforeEach(() => {
  vi.resetModules();
  storage.values.clear();
  storage.fail = false;
});
it("serializes concurrent collection writes without losing either one", async () => {
  const backend = await import("./backend");
  await Promise.all([
    backend.call("store_put", {
      collection: "groups",
      items: { a: { id: "a", name: "A", color: "blue", order: 1 } },
    }),
    backend.call("store_put", {
      collection: "groups",
      items: { b: { id: "b", name: "B", color: "red", order: 2 } },
    }),
  ]);
  const result = await backend.call<{ doc: ReturnType<typeof emptyDocument> }>(
    "store_get",
    {},
  );
  expect(Object.keys(result.doc.groups)).toEqual(["a", "b"]);
});
it("rejects failed saves, keeps the durable snapshot and allows retry", async () => {
  const backend = await import("./backend");
  storage.fail = true;
  await expect(
    backend.call("store_put", {
      collection: "groups",
      items: { a: { name: "unsaved" } },
    }),
  ).rejects.toThrow("disk full");
  const snapshot = await backend.call<{
    doc: ReturnType<typeof emptyDocument>;
  }>("store_get", {});
  expect(snapshot.doc.groups).toEqual({});
  storage.fail = false;
  await backend.call("store_put", {
    collection: "groups",
    items: { b: { name: "saved" } },
  });
  expect(storage.values.get("fluentcal.mobile.doc.v1")).toContain("saved");
});
it("recovers a corrupt primary document from the previous valid snapshot", async () => {
  storage.values.set("fluentcal.mobile.doc.v1", "{broken");
  storage.values.set(
    "fluentcal.mobile.previous.v1",
    JSON.stringify(emptyDocument()),
  );
  const backend = await import("./backend");
  const snapshot = await backend.call<{ recovered: string }>("store_get", {});
  expect(snapshot.recovered).toBe("last-saved.json");
});

it("persists tombstones across restart and rejects stale remote resurrection", async () => {
  let backend = await import("./backend");
  const group = { id: "g", name: "A", color: "blue", order: 1 };
  await backend.call("store_put", { collection: "groups", items: { g: group } });
  const before = await backend.snapshot();
  await backend.call("store_put", { collection: "groups", items: { g: null } });
  vi.resetModules();
  backend = await import("./backend");
  await backend.absorb(before, false);
  const after = await backend.snapshot();
  expect(after.groups.g).toBeUndefined();
  expect(after.meta!.groups!.g).toBeGreaterThan(before.meta!.groups!.g);
});
it("first cloud replacement preserves edits made after its baseline and local settings", async () => {
  const backend = await import("./backend");
  const group = { id: "g", name: "A", color: "blue", order: 1 };
  await backend.call("store_put", { collection: "groups", items: { g: group } });
  const baseline = await backend.snapshot();
  await backend.call("store_put", { collection: "groups", items: { g: { ...group, name: "during request" } } });
  const remote = emptyDocument();
  remote.settings.theme.mode = "dark";
  remote.groups.remote = { ...group, id: "remote" };
  await backend.absorb(remote, true, baseline);
  const after = await backend.snapshot();
  expect(after.groups.g.name).toBe("during request");
  expect(after.groups.remote).toBeDefined();
  expect(after.settings.theme.mode).toBe("system");
});
it("imports only stamp changed items and cannot turn on imported sync credentials/targets", async () => {
  const backend = await import("./backend");
  const group = { id: "g", name: "A", color: "blue", order: 1 };
  await backend.call("store_put", { collection: "groups", items: { g: group } });
  const before = await backend.snapshot();
  const incoming = structuredClone(before);
  incoming.settings.sync.enabled = true;
  incoming.settings.sync.provider = "github";
  incoming.groups.new = { ...group, id: "new" };
  await backend.call("store_replace", { doc: incoming });
  const after = await backend.snapshot();
  expect(after.meta!.groups!.g).toBe(before.meta!.groups!.g);
  expect(after.meta!.groups!.new).toBeGreaterThan(0);
  expect(after.settings.sync.enabled).toBe(false);
});
it("keeps the original pre-sync backup across failed enable/retry attempts", async () => {
  const backend = await import("./backend");
  await backend.backupBeforeSync();
  const original = await backend.readSyncBackup();
  await backend.call("store_put", { collection: "groups", items: { g: { name: "changed" } } });
  await backend.backupBeforeSync();
  expect(await backend.readSyncBackup()).toBe(original);
});
it("publishes durable local writes after an overlapping cloud merge so the UI converges", async () => {
  const backend = await import("./backend");
  const events: any[] = [];
  backend.on("store://changed", e => events.push(e));
  const remote = emptyDocument();
  remote.groups.g = { id: "g", name: "remote", color: "blue", order: 1 };
  remote.meta = { groups: { g: Date.now() } };
  await Promise.all([
    backend.absorb(remote, false),
    backend.call("store_put", { collection: "groups", items: { g: { ...remote.groups.g, name: "local edit" } } }),
  ]);
  expect(events.at(-1).items.g.name).toBe("local edit");
  expect((await backend.snapshot()).groups.g.name).toBe("local edit");
});
