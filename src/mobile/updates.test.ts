import { expect, it } from "vitest";
import { checkMobileUpdate, newerVersion } from "./updates";
const release = { name: "FluentCal Android v0.3.0", tag_name: "android-latest", draft: false, prerelease: false, body: "扫码配对",
  assets: [{ name: "FluentCal-Android-0.3.0.apk", state: "uploaded", size: 123456,
    browser_download_url: "https://github.com/Lianzy-Baimiao/FluentCal/releases/download/android-latest/FluentCal-Android-0.3.0.apk" }] };
const response = (body: unknown, status = 200) => async () => ({ status, headers: {}, text: JSON.stringify(body) });
it("compares numeric version components without lexical errors or downgrades", () => {
  expect(newerVersion("0.10.0", "0.9.9")).toBe(true);
  expect(newerVersion("0.3.0", "0.3.0")).toBe(false);
  expect(newerVersion("0.3.0", "1.0.0")).toBe(false);
});
it("uses the mobile release channel and recognizes a ready APK", async () => {
  const result = await checkMobileUpdate("0.2.0", response(release));
  expect(result.available).toBe(true);
  expect(result.version).toBe("0.3.0");
  expect((await checkMobileUpdate("0.3.0", response(release))).available).toBe(false);
});
it.each([{ ...release, name: "FluentCal 0.3.7 Windows" }, { ...release, tag_name: "v0.3.7" }, { ...release, assets: [] },
  { ...release, assets: [{ ...release.assets[0], browser_download_url: "https://evil.example/app.apk" }] }, { ...release, draft: true }, { ...release, prerelease: true }])
  ("never presents an incomplete/unrelated/untrusted release as an update", async data => {
    await expect(checkMobileUpdate("0.2.0", response(data))).rejects.toThrow();
  });
it.each([404, 429, 500])("does not claim up to date when GitHub returns %s", async status => {
  await expect(checkMobileUpdate("0.3.0", response({}, status))).rejects.toThrow();
});
