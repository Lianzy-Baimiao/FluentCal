import { Capacitor, registerPlugin } from "@capacitor/core";
import type { Settings } from "@/types";
import { FILE_NAME, MAX_BYTES, SyncError, type Remote, type RemoteFile } from "./sync-model";

export type SyncConfig = Settings["sync"];
export interface Request { url: string; method: string; headers?: Record<string, string>; body?: string }
export interface Response { status: number; headers: Record<string, string>; text: string }
export type Transport = (request: Request) => Promise<Response>;
const nativeHttp = registerPlugin<{ request(options: Request): Promise<Response> }>("SyncHttp");
export const http: Transport = async req => {
  try {
    let result: Response;
    if (Capacitor.isNativePlatform()) {
      result = await nativeHttp.request(req);
    } else {
      const r = await fetch(req.url, { method: req.method, headers: req.headers, body: req.body,
        signal: AbortSignal.timeout(30_000), redirect: "error", credentials: "omit", cache: "no-store" });
      result = { status: r.status, headers: Object.fromEntries(r.headers.entries()), text: await r.text() };
    }
    if (new TextEncoder().encode(result.text).length > MAX_BYTES * 1.5) throw new SyncError("remote", "云端响应过大，已停止同步");
    result.headers = Object.fromEntries(Object.entries(result.headers).map(([k, v]) => [k.toLowerCase(), v]));
    if (result.status >= 300 && result.status < 400 && !(req.method === "MKCOL" && result.status === 301)) throw new SyncError("config", "服务器要求跳转，请填写最终 HTTPS 地址");
    return result;
  } catch (e) {
    if (e instanceof SyncError) throw e;
    if ((e as { code?: string })?.code === "TOO_LARGE") throw new SyncError("remote", "云端响应超过大小限制，已停止同步");
    if ((e as { code?: string })?.code === "BAD_ENCODING") throw new SyncError("remote", "云端文件不是有效的 UTF-8，已停止同步");
    // Native errors can contain URLs/tokens. Never propagate provider or transport text.
    throw new SyncError("network", "连接失败或超时，请检查网络；恢复连接后会重试");
  }
};
export function toBase64(value: string) {
  const bytes = new TextEncoder().encode(value);
  let binary = "";
  for (let i = 0; i < bytes.length; i += 8192) binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
  return btoa(binary);
}
function fromBase64(value: string) {
  try { return new TextDecoder("utf-8", { fatal: true }).decode(Uint8Array.from(atob(value.replace(/\s/g, "")), c => c.charCodeAt(0))); }
  catch { throw new SyncError("remote", "云端文件编码无效，已停止同步"); }
}
function failure(status: number): never {
  if (status === 401 || status === 403) throw new SyncError("auth", "认证或权限不足，请检查密码/令牌及文件写入权限");
  if ([409, 412].includes(status)) throw new SyncError("conflict", "云端文件刚被其他设备修改");
  if ([405, 501].includes(status)) throw new SyncError("config", "服务器不支持此操作，请检查 WebDAV 地址或仓库路径");
  throw new SyncError("server", `云端服务返回 ${status}，稍后自动重试`);
}
function segments(path: string) {
  const parts = path.replace(/\\/g, "/").split("/").map(s => s.trim()).filter(Boolean);
  if (parts.some(p => p === "." || p === "..")) throw new SyncError("config", "文件路径不能包含 . 或 ..");
  return parts.map(encodeURIComponent);
}
function webdav(config: SyncConfig, secret: string, request: Transport): Remote {
  let base: URL;
  try { base = new URL(config.webdav.url.trim()); }
  catch { throw new SyncError("config", "请填写完整的 WebDAV HTTPS 地址"); }
  if (base.protocol !== "https:" || base.username || base.password || base.search || base.hash)
    throw new SyncError("config", "WebDAV 请使用 HTTPS，账号密码请填写在独立输入框中，地址不要带查询参数");
  const root = base.href.replace(/\/+$/, "");
  const folders = segments(config.webdav.folder);
  const url = [root, ...folders, FILE_NAME].join("/");
  const auth: Record<string, string> = config.webdav.user.trim() || secret ? { Authorization: "Basic " + toBase64(config.webdav.user.trim() + ":" + secret) } : {};
  const put = (text: string, file: RemoteFile | null) => {
    const guard: Record<string, string> = {};
    if (file) {
      if (!file.revision || file.revision.startsWith("W/")) throw new SyncError("config", "此 WebDAV 未提供可校验的 ETag，无法安全上传；请启用 ETag 或换用 GitHub/Gitee");
      guard["If-Match"] = file.revision;
    } else guard["If-None-Match"] = "*";
    return request({ url, method: "PUT", headers: { ...auth, ...guard, "Content-Type": "application/json; charset=utf-8" }, body: text });
  };
  return {
    async fetch() {
      const r = await request({ url, method: "GET", headers: { ...auth, "Cache-Control": "no-cache" } });
      if ([404, 409, 410].includes(r.status)) return null;
      if (r.status !== 200) failure(r.status);
      return { text: r.text, revision: r.headers.etag ?? null };
    },
    async put(text, file) {
      let r = await put(text, file);
      if ([404, 409].includes(r.status)) {
        for (let i = 1; i <= folders.length; i++) {
          const mk = await request({ url: [root, ...folders.slice(0, i)].join("/") + "/", method: "MKCOL", headers: auth });
          if (!(mk.status >= 200 && mk.status < 300) && ![301, 405].includes(mk.status)) failure(mk.status);
        }
        r = await put(text, file);
      }
      if (r.status < 200 || r.status >= 300) failure(r.status);
    },
  };
}
function parseJSON(text: string): Record<string, any> {
  try { return JSON.parse(text); }
  catch { throw new SyncError("server", "云端服务返回了无效响应，稍后自动重试"); }
}
function git(config: SyncConfig, secret: string, request: Transport): Remote {
  const provider = config.provider as "github" | "gitee";
  if (!secret.trim()) throw new SyncError("auth", "请填写具有私有仓库文件读写权限的令牌");
  const settings = config[provider];
  const repo = settings.repo.trim().replace(new RegExp(`^https?://${provider}\\.com/`, "i"), "").replace(/\/$/, "").replace(/\.git$/, "");
  if (!/^[a-zA-Z0-9_.-]+\/[a-zA-Z0-9_.-]+$/.test(repo) || repo.split("/").some(s => [".", ".."].includes(s)))
    throw new SyncError("config", "仓库请填写 owner/repo 或对应平台的仓库网址");
  const branch = settings.branch.trim();
  if (/\s/.test(branch)) throw new SyncError("config", "分支名不能含空白字符");
  const path = segments(settings.path || FILE_NAME).join("/");
  if (!path) throw new SyncError("config", "请填写同步文件路径");
  const root = (provider === "github" ? "https://api.github.com" : "https://gitee.com/api/v5") + "/repos/" + repo;
  const headers: Record<string, string> = provider === "github" ? {
    Authorization: "Bearer " + secret.trim(), Accept: "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28",
  } : { Accept: "application/json" };
  const getURL = (suffix: string, ref = false) => {
    const u = new URL(root + suffix);
    if (provider === "gitee") u.searchParams.set("access_token", secret.trim());
    if (ref && branch) u.searchParams.set("ref", branch);
    return u.href;
  };
  async function check() {
    const r = await request({ url: getURL(""), method: "GET", headers });
    if (r.status === 404) throw new SyncError("config", "找不到仓库，请检查仓库名和令牌授权范围");
    if (r.status !== 200) failure(r.status);
    const repoInfo = parseJSON(r.text);
    if (repoInfo?.private !== true) throw new SyncError("config", "日程只允许同步到私有仓库，请先设为私有");
    if (repoInfo.permissions?.push === false || repoInfo.permission?.push === false)
      throw new SyncError("auth", "令牌没有仓库写入权限");
  }
  return {
    async fetch() {
      await check();
      const r = await request({ url: getURL("/contents/" + path, true), method: "GET", headers });
      if (r.status === 404) return null;
      if (r.status !== 200) failure(r.status);
      const body = parseJSON(r.text);
      if (Array.isArray(body)) {
        if (provider === "gitee" && !body.length) return null;
        throw new SyncError("config", "同步路径指向了文件夹，请填写 JSON 文件路径");
      }
      if (typeof body?.sha !== "string" || !body.sha) throw new SyncError("remote", "云端文件缺少版本标识，已停止同步");
      if (Number(body.size) > MAX_BYTES) throw new SyncError("remote", "云端文件超过 20 MB，已停止同步");
      let text: string;
      if (typeof body.content === "string" && (body.content || !body.size)) text = fromBase64(body.content);
      else {
        const raw = await request({ url: getURL((provider === "github" ? "/contents/" : "/raw/") + path, true), method: "GET",
          headers: { ...headers, Accept: provider === "github" ? "application/vnd.github.raw+json" : "text/plain" } });
        if (raw.status !== 200) failure(raw.status);
        text = raw.text;
      }
      return { text, revision: body.sha };
    },
    async put(text, base) {
      // A repository can become public while this client is open. Check again before writes.
      await check();
      const body: Record<string, unknown> = { message: "FluentCal: sync Android calendar", content: toBase64(text) };
      if (branch) body.branch = branch;
      if (base?.revision) body.sha = base.revision;
      if (provider === "gitee") body.access_token = secret.trim();
      const r = await request({ url: root + "/contents/" + path, method: provider === "gitee" && !base ? "POST" : "PUT",
        headers: { ...headers, "Content-Type": "application/json" }, body: JSON.stringify(body) });
      if ([400, 422].includes(r.status)) {
        if (/sha|exist|match|版本|已存在/i.test(r.text)) throw new SyncError("conflict", "云端文件刚被其他设备修改");
        throw new SyncError("config", "仓库拒绝写入，请检查分支、文件路径和令牌权限");
      }
      if (![200, 201].includes(r.status)) failure(r.status);
    },
  };
}
export function createRemote(config: SyncConfig, secret: string, transport: Transport = http): Remote {
  if (config.provider === "webdav") return webdav(config, secret, transport);
  if (config.provider === "github" || config.provider === "gitee") return git(config, secret, transport);
  throw new SyncError("config", "安卓支持 WebDAV、GitHub、Gitee，请选择同步服务");
}
