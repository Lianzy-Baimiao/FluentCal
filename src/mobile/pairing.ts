import { Capacitor, registerPlugin } from "@capacitor/core";
import { DEFAULT_SETTINGS, mergeDefaults } from "@/defaults";
import { http, createRemote, type SyncConfig, type Transport } from "./sync-remote";
const scanner = registerPlugin<{ scan(): Promise<{ text: string | null }> }>("PairScanner");
export const isPrivateIPv4 = (value: string) => {
  if (!/^(\d{1,3}\.){3}\d{1,3}$/.test(value)) return false;
  const p = value.split(".").map(Number);
  if (p.some((v, i) => v > 255 || String(v) !== value.split(".")[i])) return false;
  return p[0] === 10 || (p[0] === 192 && p[1] === 168) || (p[0] === 172 && p[1] >= 16 && p[1] <= 31);
};
const bytes = (s: string) => Uint8Array.from(atob(s.replace(/-/g, "+").replace(/_/g, "/")), c => c.charCodeAt(0));
export function parsePairCode(text: string, now = Date.now()) {
  if (text.length > 2048) throw new Error("不是 FluentCal 配对二维码");
  let u: URL;
  try { u = new URL(text); } catch { throw new Error("不是 FluentCal 配对二维码"); }
  const q = u.searchParams;
  const host = q.get("host") ?? "", port = Number(q.get("port")), id = q.get("id") ?? "", key = q.get("key") ?? "", expires = Number(q.get("expires"));
  if (u.protocol !== "fluentcal:" || u.hostname !== "pair" || (u.pathname && u.pathname !== "/") || u.username || u.password || u.hash || q.get("v") !== "1" ||
      !isPrivateIPv4(host) || !Number.isInteger(port) || port < 1024 || port > 65535 || !/^[a-f0-9]{32}$/.test(id) || !/^[\w-]{43}$/.test(key))
    throw new Error("不是受支持的 FluentCal 配对码，请在电脑版「同步」页重新生成");
  if (!Number.isSafeInteger(expires) || expires <= now || expires > now + 6 * 60_000)
    throw new Error("配对码已过期，或手机与电脑时间不一致，请重新生成");
  return { url: `http://${host}:${port}/pair/${id}`, id, key, expires };
}
export interface PairResult { device: string; config: SyncConfig; secret: string }
export async function connectPairCode(text: string, transport: Transport = http): Promise<PairResult> {
  const code = parsePairCode(text);
  let response;
  try { response = await transport({ url: code.url, method: "GET" }); }
  catch { throw new Error("连接电脑失败，请连接同一 Wi-Fi，保持电脑版配对页面打开，并允许专用网络防火墙访问"); }
  if (response.status !== 200) throw new Error("配对码已使用或过期，请在电脑重新生成");
  if (response.text.length > 20_000) throw new Error("配对响应无效");
  let data: any;
  try {
    const envelope = JSON.parse(response.text);
    if (envelope.v !== 1 || typeof envelope.iv !== "string" || typeof envelope.data !== "string") throw new Error();
    const iv = bytes(envelope.iv);
    if (iv.length !== 12) throw new Error();
    const key = await crypto.subtle.importKey("raw", bytes(code.key), { name: "AES-GCM" }, false, ["decrypt"]);
    const plain = await crypto.subtle.decrypt({ name: "AES-GCM", iv, additionalData: new TextEncoder().encode(code.id) }, key, bytes(envelope.data));
    data = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(plain));
  } catch { throw new Error("配对数据校验失败，请重新扫描电脑上的二维码"); }
  if (data.app !== "FluentCal" || data.format !== 1 || data.expires !== code.expires || data.expires <= Date.now() ||
      typeof data.secret !== "string" || data.secret.length > 2048 || typeof data.device !== "string" || data.device.length > 256 ||
      !["webdav", "github", "gitee"].includes(data.sync?.provider)) throw new Error("配对数据无效或已过期");
  const provider = data.sync.provider as "webdav" | "github" | "gitee";
  const target = data.sync[provider];
  const fields = provider === "webdav" ? ["url", "user", "folder"] : ["repo", "branch", "path"];
  if (!target || fields.some(k => typeof target[k] !== "string" || target[k].length > 2048)) throw new Error("电脑同步配置不完整");
  const config = mergeDefaults(DEFAULT_SETTINGS, { sync: { provider, [provider]: Object.fromEntries(fields.map(k => [k, target[k]])),
    interval: [0, 1, 5, 15, 30, 60].includes(data.sync.interval) ? data.sync.interval : 5, enabled: false, proxy: "" } }).sync;
  createRemote(config, data.secret);
  return { device: data.device, config, secret: data.secret };
}
export async function scanPairCode() {
  if (!Capacitor.isNativePlatform()) throw new Error("请在安卓客户端中使用摄像头扫码");
  try { return (await scanner.scan()).text; }
  catch { throw new Error("无法打开相机，请在系统设置允许 FluentCal 使用相机后重试"); }
}
