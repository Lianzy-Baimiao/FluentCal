import { Capacitor, registerPlugin } from "@capacitor/core";
import { SyncError } from "./sync-model";

interface VaultPlugin {
  get(options: { key: string }): Promise<{ value: string }>;
  set(options: { key: string; value: string }): Promise<void>;
}
const native = registerPlugin<VaultPlugin>("SyncVault");
const preview = new Map<string, string>();
const allowed = new Set(["webdav", "github", "gitee"]);
export async function getSecret(key: string) {
  if (!allowed.has(key)) throw new SyncError("config", "请选择同步服务");
  try {
    return Capacitor.isNativePlatform() ? (await native.get({ key })).value : (preview.get(key) ?? "");
  } catch {
    throw new SyncError("auth", "无法读取此手机上的同步凭据，请重新填写密码或令牌");
  }
}
export async function setSecret(key: string, value: string) {
  if (!allowed.has(key)) throw new SyncError("config", "请选择同步服务");
  try {
    if (Capacitor.isNativePlatform()) await native.set({ key, value });
    else preview.set(key, value); // Browser preview never persists real secrets in localStorage.
  } catch {
    throw new SyncError("auth", "无法安全保存同步凭据，请检查设备锁屏及存储空间");
  }
}
