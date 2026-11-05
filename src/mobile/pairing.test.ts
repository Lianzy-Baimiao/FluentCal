import { expect, it, vi } from "vitest";
import { connectPairCode, isPrivateIPv4, parsePairCode } from "./pairing";
import fixtureV1 from "../../native/core/tests/fixtures/pairing-v1.json";
const id = "a".repeat(32), key = new Uint8Array(32).fill(7);
const b64 = (b: Uint8Array) => btoa(String.fromCharCode(...b)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=/g, "");
const code = (expires = Date.now() + 60_000) => `fluentcal://pair?v=1&host=192.168.1.2&port=18766&id=${id}&key=${b64(key)}&expires=${expires}`;
it("decrypts the shared Rust/WebCrypto protocol fixture", async () => {
  const now = vi.spyOn(Date, "now").mockReturnValue(fixtureV1.now);
  try {
    const result = await connectPairCode(fixtureV1.qr, async () => ({ status: 200, headers: {}, text: JSON.stringify(fixtureV1.envelope) }));
    expect(result.device).toBe("TEST-PC");
    expect(result.secret).toBe("test-only-not-a-real-password");
  } finally { now.mockRestore(); }
});
async function encrypted(change: Record<string, unknown> = {}, aad = id) {
  const expires = Date.now() + 60_000;
  const data = { app: "FluentCal", format: 1, expires, device: "OFFICE-PC", secret: "secret-test",
    sync: { provider: "webdav", interval: 5, webdav: { url: "https://dav.example/dav", user: "me", folder: "FluentCal" } }, ...change };
  const cryptoKey = await crypto.subtle.importKey("raw", key, "AES-GCM", false, ["encrypt"]);
  const iv = new Uint8Array(12).fill(3);
  const ciphertext = await crypto.subtle.encrypt({ name: "AES-GCM", iv, additionalData: new TextEncoder().encode(aad) }, cryptoKey, new TextEncoder().encode(JSON.stringify(data)));
  return { qr: code(expires), response: { status: 200, headers: {}, text: JSON.stringify({ v: 1, iv: b64(iv), data: b64(new Uint8Array(ciphertext)) }) } };
}
it.each(["127.0.0.1", "8.8.8.8", "169.254.169.254", "192.168.1.999", "010.1.2.3", "localhost", "192.168.0.1.evil.com"])("rejects non-LAN literal %s", ip => {
  expect(isPrivateIPv4(ip)).toBe(false);
  expect(() => parsePairCode(code().replace("192.168.1.2", ip))).toThrow();
});
it.each(["10.1.2.3", "192.168.0.1", "172.16.1.2", "172.31.255.1"])("accepts private IPv4 %s", ip => expect(isPrivateIPv4(ip)).toBe(true));
it("rejects expired, far-future and non-FluentCal QR codes before any request", () => {
  expect(() => parsePairCode(code(Date.now() - 1))).toThrow("过期");
  expect(() => parsePairCode(code(Date.now() + 86400000))).toThrow();
  expect(() => parsePairCode("https://example.com")).toThrow();
  expect(() => parsePairCode(code().replace("v=1", "v=2"))).toThrow();
});
it("decrypts authenticated config without including keys in the HTTP request", async () => {
  const fixture = await encrypted();
  const request = vi.fn(async () => fixture.response);
  const result = await connectPairCode(fixture.qr, request);
  expect(result.device).toBe("OFFICE-PC");
  expect(result.secret).toBe("secret-test");
  expect(result.config.provider).toBe("webdav");
  expect(result.config.enabled).toBe(false);
  expect(request.mock.calls[0]).toEqual([{ url: `http://192.168.1.2:18766/pair/${id}`, method: "GET" }]);
});
it("rejects tampered ciphertext or a mismatching pairing id", async () => {
  const fixture = await encrypted({}, "wrong-id");
  await expect(connectPairCode(fixture.qr, async () => fixture.response)).rejects.toThrow("校验失败");
});
it("does not follow expired or already consumed sessions", async () => {
  await expect(connectPairCode(code(), async () => ({ status: 410, headers: {}, text: "" }))).rejects.toThrow("已使用或过期");
});
it("validates decrypted targets and ignores unrelated credential/settings fields", async () => {
  const bad = await encrypted({ sync: { provider: "folder" } });
  await expect(connectPairCode(bad.qr, async () => bad.response)).rejects.toThrow("无效");
  const good = await encrypted({ sync: { provider: "github", proxy: "evil", interval: -1, enabled: true,
    github: { repo: "owner/repo", branch: "", path: "" }, webdav: { user: "unrelated" } } });
  const result = await connectPairCode(good.qr, async () => good.response);
  expect(result.config.webdav.user).toBe("");
  expect(result.config.proxy).toBe("");
  expect(result.config.interval).toBe(5);
});
