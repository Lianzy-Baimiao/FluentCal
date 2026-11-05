<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Capacitor } from "@capacitor/core";
import type { SyncProbe } from "@/api";
import { doc, flushSettings } from "@/stores/doc";
import { call } from "@/api";
import { readSyncBackup } from "@/mobile/backend";
import { disableSync, enableSync, probeSync, saveSyncConfig, syncNow, syncStatus, updateSecret } from "@/mobile/sync";
import { getSecret, setSecret } from "@/mobile/vault";
import type { SyncConfig } from "@/mobile/sync-remote";
import { connectPairCode, scanPairCode } from "@/mobile/pairing";

const draft = ref<SyncConfig>(JSON.parse(JSON.stringify(doc.settings.sync)));
if (!["webdav", "github", "gitee"].includes(draft.value.provider)) draft.value.provider = "webdav";
const password = ref("");
const storedSecret = ref(false);
const busy = ref(false);
const error = ref("");
const message = ref("");
const probe = ref<SyncProbe>();
const expanded = ref(false);
const backup = ref(false);
const gitSettings = computed(() => draft.value.provider === "gitee" ? draft.value.gitee : draft.value.github);
const statusText = computed(() => syncStatus.syncing ? "正在同步…" : syncStatus.error ? (syncStatus.halted ? "同步已暂停" : "等待自动重试") : !syncStatus.enabled ? "尚未开启" : syncStatus.pending ? "本机有修改，等待上传" : "已同步");
const formatTime = (at: number | null) => at ? new Date(at).toLocaleString("zh-CN", { hour12: false }) : "尚无记录";
watch(() => draft.value.provider, async key => {
  password.value = "";
  storedSecret.value = false;
  try { storedSecret.value = !!await getSecret(key); } catch { /* Allow re-entering an invalidated credential. */ }
}, { immediate: true });
watch(draft, () => { probe.value = undefined; }, { deep: true });
watch(password, () => { probe.value = undefined; });
void readSyncBackup().then(value => { backup.value = !!value; });
async function action(fn: () => Promise<unknown>) {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  message.value = "";
  try { await fn(); } catch (e) { error.value = e instanceof Error ? e.message : "操作失败，请重试"; }
  finally { busy.value = false; }
}
async function test() {
  await action(async () => {
    await flushSettings();
    await saveSyncConfig(draft.value, password.value || undefined);
    password.value = "";
    storedSecret.value = !!await getSecret(draft.value.provider);
    probe.value = await probeSync();
  });
}
function scan() {
  void action(async () => {
    const code = await scanPairCode();
    if (!code) return;
    const result = await connectPairCode(code);
    await saveSyncConfig(result.config, result.secret);
    draft.value = JSON.parse(JSON.stringify(result.config));
    password.value = "";
    storedSecret.value = !!result.secret;
    probe.value = await probeSync();
    message.value = "已连接电脑「" + result.device + "」，配置已安全保存，请选择首次同步方式";
  });
}
function enable(mode: "merge" | "replace") {
  void action(async () => {
    try { await enableSync(mode); }
    finally { backup.value = !!await readSyncBackup(); }
    probe.value = undefined;
    expanded.value = false;
    message.value = "自动同步已开启";
  });
}
function disable() {
  void action(async () => {
    await disableSync();
    draft.value = JSON.parse(JSON.stringify(doc.settings.sync));
    expanded.value = true;
    probe.value = undefined;
    message.value = "已关闭自动同步，本机和云端数据均已保留";
  });
}
function renew() {
  void action(async () => {
    await updateSecret(password.value);
    password.value = "";
    storedSecret.value = true;
    await syncNow();
  });
}
function forgetSecret() {
  void action(async () => {
    await setSecret(draft.value.provider, "");
    storedSecret.value = false;
    password.value = "";
    probe.value = undefined;
    message.value = "此手机保存的凭据已清除";
  });
}
function exportBackup() {
  void action(async () => {
    const content = await readSyncBackup();
    if (!content) throw new Error("暂无首次同步前备份");
    await call("file_save", { content, defaultName: "fluentcal-before-sync.json" });
    message.value = "备份已导出，可通过「导入备份」找回原有任务";
  });
}
</script>

<template>
  <section class="sync-card" aria-label="自动云同步">
    <div class="sync-heading"><div><h3>自动云同步</h3><p>与电脑共用一个云端文件</p></div><span class="badge" :class="{ good: syncStatus.enabled && !syncStatus.error, bad: syncStatus.halted }">{{ statusText }}</span></div>
    <div class="sync-info" aria-live="polite">
      <p v-if="syncStatus.enabled">{{ syncStatus.provider }} · 上次成功 {{ formatTime(syncStatus.lastSync) }}</p>
      <p v-if="syncStatus.remoteDevice">云端设备：{{ syncStatus.remoteDevice }}</p>
      <p v-if="syncStatus.error" class="error" role="alert">{{ syncStatus.error }}</p>
    </div>
    <div v-if="syncStatus.enabled" class="buttons">
      <button class="primary" :disabled="busy || syncStatus.syncing" @click="action(() => syncNow())">立即同步</button>
      <button :disabled="busy" @click="expanded = !expanded">{{ expanded ? '收起配置' : '查看配置' }}</button>
      <button :disabled="busy || syncStatus.syncing" @click="disable">关闭同步</button>
    </div>
    <div v-else class="pair-entry">
      <p>在电脑 FluentCal 的「设置 → 同步」生成二维码，用手机扫一扫，即可带入配置。</p>
      <div class="buttons"><button class="primary" :disabled="busy" @click="scan">{{ busy ? '正在连接…' : '扫码连接电脑' }}</button><button :disabled="busy" @click="expanded = !expanded">{{ expanded ? '收起手动设置' : '手动配置' }}</button></div>
      <p class="hint">手机与电脑需在同一局域网。扫码后直接连接云端，日常同步不需要电脑一直在线。</p>
    </div>
    <form v-if="expanded" @submit.prevent="test">
      <fieldset :disabled="busy || syncStatus.enabled">
        <label>同步服务<select v-model="draft.provider" aria-label="同步服务"><option value="webdav">WebDAV</option><option value="github">GitHub 私有仓库</option><option value="gitee">Gitee 私有仓库</option></select></label>
        <template v-if="draft.provider === 'webdav'">
          <label>WebDAV 地址<input v-model="draft.webdav.url" aria-label="WebDAV 地址" type="url" placeholder="https://dav.example.com/dav" autocapitalize="off" spellcheck="false" /></label>
          <label>账号<input v-model="draft.webdav.user" aria-label="WebDAV 账号" autocomplete="username" autocapitalize="off" spellcheck="false" /></label>
          <label>云端文件夹<input v-model="draft.webdav.folder" aria-label="云端文件夹" placeholder="FluentCal" autocapitalize="off" spellcheck="false" /></label>
        </template>
        <template v-else>
          <label>私有仓库<input v-model="gitSettings.repo" aria-label="私有仓库" placeholder="owner/repo" autocapitalize="off" spellcheck="false" /></label>
          <div class="two-fields"><label>分支<input v-model="gitSettings.branch" aria-label="分支" placeholder="默认分支" autocapitalize="off" spellcheck="false" /></label><label>文件路径<input v-model="gitSettings.path" aria-label="文件路径" placeholder="fluentcal-data.json" autocapitalize="off" spellcheck="false" /></label></div>
        </template>
        <label>自动检查间隔<select v-model.number="draft.interval" aria-label="自动检查间隔"><option :value="1">1 分钟</option><option :value="5">5 分钟</option><option :value="15">15 分钟</option><option :value="30">30 分钟</option><option :value="60">60 分钟</option><option :value="0">仅启动、恢复前台和修改后</option></select></label>
      </fieldset>
      <label>{{ draft.provider === 'webdav' ? '密码 / 应用密码' : '访问令牌' }}<input v-model="password" :disabled="busy" :aria-label="draft.provider === 'webdav' ? '同步密码' : '访问令牌'" type="password" autocomplete="new-password" :placeholder="storedSecret ? '已安全保存，留空保留' : '填写后保存在此手机'" /></label>
      <p class="hint">{{ Capacitor.isNativePlatform() ? '凭据由 Android Keystore 加密，不进入备份或云端。' : '浏览器预览的凭据只在当前页面内存中保留，刷新后需重新输入。' }}{{ draft.provider === 'webdav' ? '使用与电脑相同的 HTTPS 地址和文件夹。' : '使用与电脑相同的私有仓库、分支和路径；令牌需有文件读写权限。' }}</p>
      <button v-if="!syncStatus.enabled" class="primary" type="submit" :disabled="busy">{{ busy ? '正在连接…' : '保存并测试连接' }}</button>
      <button v-else type="button" :disabled="busy || !password" @click="renew">更新凭据并重试</button>
      <button v-if="storedSecret && !syncStatus.enabled" type="button" :disabled="busy" @click="forgetSecret">清除已存凭据</button>
    </form>
    <div v-if="probe" class="first-sync" aria-label="首次同步选择">
      <h4>{{ probe.exists ? '发现云端日历' : '连接成功，云端暂无日历' }}</h4>
      <p v-if="probe.exists">{{ probe.tasks }} 个任务 · {{ probe.groups }} 个分组<br />{{ probe.device || '未知设备' }} · {{ formatTime(probe.savedAt) }}</p>
      <p>同步前自动保存一份本机备份。合并会保留两边的任务，同一项取修改时间较新的一方。</p>
      <p v-if="probe.exists">「使用云端日历」会用云端任务、分组和日期标记替换手机现有内容，适合清除手机示例后接入电脑日历。</p>
      <div class="buttons"><button class="primary" :disabled="busy" @click="enable('merge')">{{ probe.exists ? '合并并开启同步' : '上传本机并开启同步' }}</button><button v-if="probe.exists" :disabled="busy" @click="enable('replace')">使用云端日历</button></div>
    </div>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <p v-if="message" class="message" role="status">{{ message }}</p>
    <button v-if="backup" class="backup" :disabled="busy" @click="exportBackup">导出首次同步前备份</button>
    <p class="hint footer">开启后，启动、恢复前台和联网时自动同步；本机修改后 WebDAV 约 2 秒上传，GitHub/Gitee 约 1 分钟合并提交。定时检查在应用运行时生效，系统挂起或结束应用后，下次打开自动补同步。</p>
  </section>
</template>

<style scoped>
.sync-card { margin: 14px 0; padding: 16px; border: 1px solid var(--card-stroke); border-radius: 15px; background: var(--card-fill); }
.pair-entry { margin-top: 12px; font-size: 13px; }
.sync-heading { display: flex; justify-content: space-between; align-items: flex-start; gap: 8px; }
h3 { font-size: 16px; margin: 0 0 4px; } h4 { margin: 0 0 8px; }
p { margin: 5px 0; line-height: 1.65; overflow-wrap: anywhere; }
.sync-heading p, .sync-info { font-size: 12px; color: var(--text-secondary); }
.badge { flex: none; max-width: 48%; padding: 5px 8px; border-radius: 7px; font-size: 11px; background: var(--subtle-fill-hover); }
.badge.good { color: var(--accent-text); background: color-mix(in srgb, var(--accent-fill) 12%, transparent); }
.badge.bad, .error { color: var(--critical); }
fieldset { border: 0; padding: 0; margin: 0; min-width: 0; }
form { margin-top: 12px; } label { display: flex; flex-direction: column; gap: 6px; margin-bottom: 12px; font-size: 12px; color: var(--text-secondary); }
input, select { box-sizing: border-box; width: 100%; min-width: 0; min-height: 43px; border: 1px solid var(--card-stroke); border-radius: 8px; padding: 9px 10px; background: var(--solid-base); color: var(--text-primary); font: inherit; font-size: 14px; }
input:focus, select:focus { outline: 2px solid var(--accent-fill); outline-offset: 1px; }
.two-fields { display: grid; grid-template-columns: 1fr 1.5fr; gap: 10px; }
.buttons { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 10px; }
button { border: 1px solid var(--card-stroke); background: var(--subtle-fill-hover); color: var(--text-primary); border-radius: 8px; padding: 10px 12px; min-height: 42px; font: inherit; font-size: 13px; cursor: pointer; }
button.primary { background: var(--accent-fill); color: white; border-color: transparent; }
button:disabled, input:disabled, select:disabled { opacity: .65; cursor: default; }
.hint { color: var(--text-secondary); font-size: 11px; margin: 8px 0 12px; }
.footer { margin-bottom: 0; margin-top: 13px; }
.first-sync { border: 1px solid var(--card-stroke); border-radius: 11px; padding: 13px; margin-top: 14px; background: color-mix(in srgb, var(--accent-fill) 5%, transparent); font-size: 12px; }
.error, .message { font-size: 12px; margin-top: 12px; }
.message { color: var(--accent-text); }
.backup { margin-top: 12px; font-size: 12px; }
</style>
