import { describe, expect, it, vi } from "vitest";
import { emptyDocument } from "./data";
import { createRemote, toBase64, type Response, type Request, type Transport } from "./sync-remote";
const reply = (status: number, text = "", headers = {}): Response => ({ status, text, headers });
function config(provider: "webdav" | "github" | "gitee" = "webdav") {
  const s = emptyDocument().settings.sync;
  s.provider = provider;
  s.webdav = { url: "https://dav.example/dav/", user: "测试", folder: "日历/手机" };
  s.github = { repo: "owner/repo", branch: "main", path: "日历/data.json" };
  s.gitee = { ...s.github };
  return s;
}
function transport(replies: Response[]) {
  return vi.fn<Transport>(async () => {
    const r = replies.shift();
    if (!r) throw new Error("Unexpected HTTP request");
    return r;
  });
}
describe("WebDAV", () => {
  it("encodes UTF-8 credentials and segments, then writes using the fetched ETag", async () => {
    const http = transport([reply(200, "{}", { etag: '"v1"' }), reply(204)]);
    const remote = createRemote(config(), "密码", http);
    const base = await remote.fetch();
    await remote.put("{}", base);
    expect(http.mock.calls[0][0].url).toBe("https://dav.example/dav/%E6%97%A5%E5%8E%86/%E6%89%8B%E6%9C%BA/fluentcal-data.json");
    expect(http.mock.calls[0][0].headers?.Authorization).toBe("Basic " + toBase64("测试:密码"));
    expect(http.mock.calls[1][0].headers?.["If-Match"]).toBe('"v1"');
  });
  it("creates missing parents in order and preserves If-None-Match on retry", async () => {
    const http = transport([reply(404), reply(409), reply(201), reply(201), reply(201)]);
    const remote = createRemote(config(), "pw", http);
    await remote.put("{}", await remote.fetch());
    expect(http.mock.calls.map(([r]) => r.method)).toEqual(["GET", "PUT", "MKCOL", "MKCOL", "PUT"]);
    expect(http.mock.calls[4][0].headers?.["If-None-Match"]).toBe("*");
  });
  it.each([null, 'W/"weak"'])("refuses unguarded overwrites with ETag %s", async revision => {
    const http = transport([]);
    await expect(createRemote(config(), "pw", http).put("{}", { text: "{}", revision })).rejects.toMatchObject({ kind: "config" });
    expect(http).not.toHaveBeenCalled();
  });
  it("classifies auth/conflict failures", async () => {
    await expect(createRemote(config(), "pw", transport([reply(401)])).fetch()).rejects.toMatchObject({ kind: "auth" });
    await expect(createRemote(config(), "pw", transport([reply(412)])).put("{}", null)).rejects.toMatchObject({ kind: "conflict" });
  });
  it.each(["http://dav.example", "https://user:pw@dav.example", "https://dav.example?token=x"])("rejects unsafe address %s", url => {
    const c = config(); c.webdav.url = url;
    expect(() => createRemote(c, "pw")).toThrow();
  });
  it("rejects path traversal", () => {
    const c = config(); c.webdav.folder = "../other";
    expect(() => createRemote(c, "pw")).toThrow();
  });
});
describe.each(["github", "gitee"] as const)("%s", provider => {
  it("checks privacy, decodes Unicode and sends a SHA-guarded update", async () => {
    const http = transport([reply(200, '{"private":true}'), reply(200, JSON.stringify({ sha: "sha1", content: toBase64("日程") })), reply(200, '{"private":true}'), reply(200)]);
    const remote = createRemote(config(provider), "token-secret", http);
    const file = await remote.fetch();
    expect(file?.text).toBe("日程");
    await remote.put("新日程", file);
    const req = http.mock.calls.at(-1)![0];
    expect(JSON.parse(req.body!).sha).toBe("sha1");
    expect(JSON.parse(req.body!).branch).toBe("main");
    expect(req.method).toBe("PUT");
    expect(req.url).not.toContain("token-secret");
    if (provider === "gitee") expect(JSON.parse(req.body!).access_token).toBe("token-secret");
    else expect(req.headers?.Authorization).toBe("Bearer token-secret");
  });
  it("refuses public repositories before fetching or writing calendar data", async () => {
    const http = transport([reply(200, '{"private":false}')]);
    await expect(createRemote(config(provider), "token", http).fetch()).rejects.toMatchObject({ kind: "config" });
    expect(http).toHaveBeenCalledTimes(1);
  });
  it("uses the provider's create verb and no SHA for missing files", async () => {
    const requests: Request[] = [];
    const http: Transport = async r => { requests.push(r); return r.method === "GET" ? reply(200, '{"private":true}') : reply(201); };
    await createRemote(config(provider), "token", http).put("{}", null);
    expect(requests[1].method).toBe(provider === "gitee" ? "POST" : "PUT");
    expect(JSON.parse(requests[1].body!)).not.toHaveProperty("sha");
  });
  it("treats sha mismatches as retriable conflicts", async () => {
    const http = transport([reply(200, '{"private":true}'), reply(422, '{"message":"sha does not match"}')]);
    await expect(createRemote(config(provider), "token", http).put("{}", { text: "{}", revision: "old" })).rejects.toMatchObject({ kind: "conflict" });
  });
  it("fetches the raw content of large files", async () => {
    const http = transport([reply(200, '{"private":true}'), reply(200, '{"sha":"sha1","size":1048577}'), reply(200, "raw content")]);
    expect((await createRemote(config(provider), "token", http).fetch())?.text).toBe("raw content");
    expect(http.mock.calls[2][0].url).toContain(provider === "github" ? "/contents/" : "/raw/");
  });
});
