import { http, type Transport } from "./sync-remote";
import { MOBILE_RELEASE } from "./version";
export interface MobileUpdate { version: string; available: boolean; url: string; size: number; notes: string }
export function versionParts(value: string): number[] {
  if (!/^\d+\.\d+\.\d+$/.test(value)) throw new Error("版本信息无效");
  const parts = value.split(".").map(Number);
  if (parts.some(n => !Number.isSafeInteger(n))) throw new Error("版本信息无效");
  return parts;
}
export function newerVersion(next: string, current: string) {
  const a = versionParts(next), b = versionParts(current);
  for (let i = 0; i < 3; i++) { if (a[i] !== b[i]) return a[i] > b[i]; }
  return false;
}
export async function checkMobileUpdate(current: string, transport: Transport = http): Promise<MobileUpdate> {
  let r;
  try { r = await transport({ url: "https://api.github.com/repos/Lianzy-Baimiao/FluentCal/releases/tags/android-latest", method: "GET",
    headers: { Accept: "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28", "Cache-Control": "no-cache" } }); }
  catch { throw new Error("无法连接 GitHub，请检查网络后重试"); }
  if (r.status === 404) throw new Error("GitHub 移动端发布暂不可用，请稍后重试");
  if ([403, 429].includes(r.status)) throw new Error("GitHub 暂时限制请求，请稍后重试或打开发布页");
  if (r.status !== 200) throw new Error("GitHub 更新检查失败，请稍后重试");
  let release;
  try { release = JSON.parse(r.text); } catch { throw new Error("GitHub 返回的更新信息无效"); }
  const version = /^FluentCal Android v(\d+\.\d+\.\d+)$/.exec(release?.name ?? "")?.[1];
  if (!version || release.tag_name !== "android-latest" || release.draft || release.prerelease || !Array.isArray(release.assets))
    throw new Error("移动端发布信息不完整，请打开 GitHub 发布页查看");
  const asset = release.assets.find((a: any) => a.name === `FluentCal-Android-${version}.apk` && a.state === "uploaded");
  const expected = `https://github.com/Lianzy-Baimiao/FluentCal/releases/download/android-latest/FluentCal-Android-${version}.apk`;
  if (!asset || asset.browser_download_url !== expected || !Number.isSafeInteger(asset.size) || asset.size <= 0)
    throw new Error("移动端安装包尚未就绪，请稍后重试");
  return { version, available: newerVersion(version, current), url: MOBILE_RELEASE, size: asset.size,
    notes: typeof release.body === "string" ? release.body.slice(0, 6000) : "" };
}
