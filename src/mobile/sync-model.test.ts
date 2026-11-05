import { describe, expect, it, vi } from "vitest";
import { emptyDocument } from "./data";
import { canonical, cloudView, decodeCloud, encodeCloud, mergeCloud, prune, SyncError, syncRound, type CloudView, type Remote, type SyncStore } from "./sync-model";

const group = (name: string) => ({ id: "g", name, color: "blue", order: 0 });
const NOW = Date.now();
function fixture(name: string, time = NOW) {
  const doc = emptyDocument();
  doc.groups.g = group(name);
  doc.meta = { groups: { g: time } };
  return doc;
}
function localStore(doc = emptyDocument()) {
  const store: SyncStore = { snapshot: async () => structuredClone(doc), absorb: async (view) => cloudView(mergeCloud(doc, view)) };
  return { doc, store };
}
describe("desktop-compatible cloud merge", () => {
  it("uses per-record timestamps, keeps ties local and retains unrelated data", () => {
    const ours = fixture("本机", NOW);
    ours.settings.theme.mode = "dark";
    const theirs = fixture("电脑", NOW + 1);
    theirs.dayMarks["2026-11-05"] = { color: "red" };
    mergeCloud(ours, cloudView(theirs));
    expect(ours.groups.g.name).toBe("电脑");
    expect(ours.settings.theme.mode).toBe("dark");
    expect(ours.dayMarks["2026-11-05"].color).toBe("red");
    mergeCloud(ours, cloudView(fixture("平局", NOW + 1)));
    expect(ours.groups.g.name).toBe("电脑");
  });
  it("propagates deletion tombstones and cannot resurrect with a stale offline copy", () => {
    const ours = fixture("旧记录");
    const deleted = fixture("删除");
    delete deleted.groups.g;
    deleted.meta!.groups!.g = NOW + 5;
    mergeCloud(ours, deleted);
    expect(ours.groups).toEqual({});
    mergeCloud(ours, fixture("离线旧记录", NOW));
    expect(ours.groups).toEqual({});
    expect(ours.meta!.groups!.g).toBe(NOW + 5);
  });
  it("keeps metadata-only updates and only expires absent tombstones after 60 days", () => {
    const doc = fixture("同值", 1);
    mergeCloud(doc, fixture("同值", NOW));
    doc.meta!.groups!.expired = NOW - 61 * 86_400_000;
    doc.meta!.groups!.recent = NOW - 59 * 86_400_000;
    prune(doc, NOW);
    expect(doc.meta!.groups).toEqual({ g: NOW, recent: NOW - 59 * 86_400_000 });
  });
  it("reads/writes the native desktop envelope without settings or credentials", () => {
    const doc = fixture("电脑");
    const encoded = encodeCloud(doc, "Android-test", NOW);
    const file = JSON.parse(encoded);
    expect(Object.keys(file.data).sort()).toEqual(["dayMarks", "groups", "meta", "tasks"]);
    expect(file.app).toBe("FluentCal");
    expect(file.format).toBe(1);
    expect(decodeCloud(encoded)?.view).toEqual(cloudView(doc));
    expect(decodeCloud(" \ufeff ")).toBeNull();
    expect(canonical({ b: 2, a: 1 })).toBe(canonical({ a: 1, b: 2 }));
  });
});
describe("guarded sync rounds", () => {
  it.each(["{broken", '{"app":"Other","format":1,"data":{}}', '{"app":"FluentCal","format":2,"data":{}}', '{"app":"FluentCal","format":1,"data":{"tasks":{},"groups":[]}}'])
    ("never uploads or absorbs invalid remote data: %s", async text => {
      const local = localStore();
      const remote = { fetch: async () => ({ text, revision: "a" }), put: vi.fn() };
      await expect(syncRound(local.store, remote, "phone")).rejects.toMatchObject({ kind: "remote" });
      expect(remote.put).not.toHaveBeenCalled();
      expect(local.doc.groups).toEqual({});
    });
  it("re-fetches and merges after a conditional-write conflict", async () => {
    const local = localStore(fixture("手机"));
    let uploaded = "";
    let calls = 0;
    const other = emptyDocument();
    other.groups.other = { ...group("电脑新建"), id: "other" };
    other.meta = { groups: { other: NOW } };
    const remote: Remote = {
      fetch: async () => calls ? { text: encodeCloud(other, "PC"), revision: "v2" } : null,
      put: async (text, base) => {
        if (calls++ === 0) throw new SyncError("conflict", "changed");
        expect(base?.revision).toBe("v2"); uploaded = text;
      },
    };
    await syncRound(local.store, remote, "phone");
    expect(Object.keys(decodeCloud(uploaded)!.view.groups).sort()).toEqual(["g", "other"]);
  });
  it("merges into the latest local document when edits happen during GET", async () => {
    const local = localStore(fixture("手机旧版"));
    const remoteDoc = fixture("桌面旧版", NOW - 1);
    let uploaded: CloudView | undefined;
    const remote: Remote = {
      fetch: async () => {
        local.doc.groups.g.name = "请求期间的修改";
        local.doc.meta!.groups!.g = NOW + 1;
        return { text: encodeCloud(remoteDoc, "PC"), revision: "v1" };
      },
      put: async text => { uploaded = decodeCloud(text)!.view; },
    };
    await syncRound(local.store, remote, "phone");
    expect(uploaded?.groups.g.name).toBe("请求期间的修改");
  });
  it("does not create Git commits when content is unchanged", async () => {
    const local = localStore(fixture("相同"));
    const remote = { fetch: async () => ({ text: encodeCloud(local.doc, "PC"), revision: "v1" }), put: vi.fn() };
    await syncRound(local.store, remote, "phone");
    expect(remote.put).not.toHaveBeenCalled();
  });
  it("bounds retries under sustained concurrent writes", async () => {
    const local = localStore(fixture("手机"));
    const remote = { fetch: vi.fn(async () => null), put: vi.fn(async () => { throw new SyncError("conflict", "busy"); }) };
    await expect(syncRound(local.store, remote, "phone")).rejects.toMatchObject({ kind: "server" });
    expect(remote.fetch).toHaveBeenCalledTimes(3);
  });
});
