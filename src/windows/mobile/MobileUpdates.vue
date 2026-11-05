<script setup lang="ts">
import { ref } from "vue";
import { App } from "@capacitor/app";
import { Capacitor } from "@capacitor/core";
import { MOBILE_VERSION, MOBILE_RELEASE } from "@/mobile/version";
import { checkMobileUpdate, type MobileUpdate } from "@/mobile/updates";
import { call } from "@/api";
const current = ref(MOBILE_VERSION);
const busy = ref(false), error = ref("");
const result = ref<MobileUpdate>();
if (Capacitor.isNativePlatform()) void App.getInfo().then(info => { current.value = info.version; }).catch(() => {});
async function check() {
  busy.value = true; error.value = ""; result.value = undefined;
  try { result.value = await checkMobileUpdate(current.value); }
  catch (e) { error.value = e instanceof Error ? e.message : "检查更新失败"; }
  finally { busy.value = false; }
}
async function open() {
  try { await call("open_url", { url: MOBILE_RELEASE }); }
  catch { error.value = "无法打开浏览器，请检查系统浏览器设置"; }
}
</script>
<template>
  <section class="update-card" aria-label="移动端更新">
    <h3>移动端更新 <span>v{{ current }}</span></h3>
    <p>通过 GitHub 获取安卓客户端的新版本</p>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <p v-if="result" role="status">{{ result.available ? `发现新版本 v${result.version} · ${(result.size / 1048576).toFixed(1)} MB` : '当前已是最新移动端版本' }}</p>
    <details v-if="result?.available && result.notes"><summary>查看更新说明</summary><p class="notes">{{ result.notes }}</p></details>
    <div><button :disabled="busy" @click="check">{{ busy ? '正在检查…' : '检查 GitHub 更新' }}</button><button @click="open">{{ result?.available ? '下载更新' : '打开 GitHub 发布页' }}</button></div>
  </section>
</template>
<style scoped>
.update-card { padding: 16px; margin: 14px 0; border: 1px solid var(--card-stroke); border-radius: 15px; background: var(--card-fill); }
h3 { margin: 0; font-size: 15px; } h3 span { float: right; font-size: 12px; color: var(--text-secondary); font-weight: 400; }
p { font-size: 12px; color: var(--text-secondary); line-height: 1.6; } .error { color: var(--critical); }
.notes { white-space: pre-wrap; overflow-wrap: anywhere; } summary { font-size: 12px; cursor: pointer; }
.update-card > div { display: flex; gap: 8px; flex-wrap: wrap; }
button { min-height: 42px; padding: 8px 12px; border: 1px solid var(--card-stroke); border-radius: 8px; background: var(--subtle-fill-hover); color: var(--text-primary); font: inherit; font-size: 12px; }
button:disabled { opacity: .6; }
</style>
