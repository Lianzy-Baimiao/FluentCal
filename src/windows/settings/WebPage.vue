<script setup lang="ts">
import { defineAsyncComponent, onMounted, ref, watch } from 'vue';
import { confirm, FButton, FInfoBar, FNumberBox, FSettingsCard, FSettingsGroup, FToggle } from '@/fluent';
import { call } from '@/api';
import { useSetting } from '@/stores/doc';

const QrCode = defineAsyncComponent(() => import('./QrCode.vue'));

interface Address {
  ip: string;
  label: string;
  virt: boolean;
  url: string;
}
interface WebInfo {
  enabled: boolean;
  running: boolean;
  port: number;
  error: string | null;
  addresses: Address[];
  localUrl: string;
}

const enabled = useSetting<boolean>('web.enabled');
const port = useSetting<number>('web.port');
const info = ref<WebInfo | null>(null);
const msg = ref<{ sev: 'success' | 'error' | 'info'; text: string } | null>(null);

const load = async () => (info.value = await call<WebInfo>('web_info'));
// The server follows the settings a moment after they are saved.
watch([enabled, port], () => setTimeout(load, 400));
onMounted(load);

async function copy(url: string) {
  try {
    await navigator.clipboard.writeText(url);
    msg.value = { sev: 'success', text: '已复制链接。链接里带访问码，只发给自己的设备。' };
  } catch {
    msg.value = { sev: 'error', text: '无法复制，请手动选择链接复制' };
  }
}

async function reset() {
  const r = await confirm({ title: '更换访问码？', content: '已经发出的链接和已打开的网页会立即失效，需要重新扫码。', primaryText: '更换' });
  if (r !== 'primary') return;
  await call('web_reset_token');
  await load();
  msg.value = { sev: 'info', text: '已更换访问码，请用新的链接或二维码打开。' };
}
</script>

<template>
  <div class="page">
    <FSettingsGroup title="网页查看">
      <FSettingsCard icon="globe" title="在手机或其他电脑的浏览器中查看" description="同一 Wi-Fi/局域网内的设备打开链接或扫二维码即可查看日历、列表和甘特图。只能查看，不能修改">
        <FToggle v-model="enabled" label="网页查看" />
      </FSettingsCard>
      <FInfoBar v-if="info?.error" severity="error" title="网页服务没有启动">{{ info.error }}</FInfoBar>
      <FInfoBar v-if="msg" :severity="msg.sev" closable @close="msg = null">{{ msg.text }}</FInfoBar>
    </FSettingsGroup>

    <FSettingsGroup v-if="enabled && info?.running" title="打开方式">
      <div class="share">
        <QrCode v-if="info.addresses.length" :text="info.addresses[0].url" label="网页查看的二维码" />
        <div class="links">
          <p class="note">{{ info.addresses.length ? '用手机相机或微信扫码，或在浏览器中打开：' : '这台电脑没有连接局域网，只能在本机浏览器中查看。' }}</p>
          <div v-for="a in info.addresses" :key="a.ip" class="link">
            <code>http://{{ a.ip }}:{{ info.port }}</code>
            <span class="note">{{ a.label }}{{ a.virt ? '（虚拟网卡，手机一般连不上）' : '' }}</span>
            <FButton size="small" @click="copy(a.url)">复制链接</FButton>
          </div>
          <div class="row-ctl start">
            <FButton size="small" variant="subtle" icon="globe" @click="call('open_url', { url: info.localUrl })">在本机浏览器中打开</FButton>
            <FButton size="small" variant="subtle" icon="key" @click="reset">更换访问码</FButton>
          </div>
        </div>
      </div>
    </FSettingsGroup>

    <FSettingsGroup title="高级">
      <FSettingsCard icon="settings" title="端口" description="被其他程序占用时换一个（1024–65535）">
        <FNumberBox v-model="port" :min="1024" :max="65535" :width="120" label="端口" />
      </FSettingsCard>
    </FSettingsGroup>
    <ul class="note list">
      <li>第一次开启时 Windows 防火墙会询问是否允许 FluentCal 访问网络，请勾选“专用网络”并允许，否则手机连不上。</li>
      <li>链接里带有访问码，知道链接的人都能查看你的日程；不小心发给了别人就点“更换访问码”。</li>
      <li>局域网内传输没有加密，只在家里、公司等信任的网络中使用。</li>
      <li>网页由这台电脑上运行的 FluentCal 提供：电脑关机或退出 FluentCal 后打不开。在电脑上的修改几秒内会出现在网页上。</li>
    </ul>
  </div>
</template>

<style scoped>
.share {
  display: flex;
  gap: 20px;
  align-items: flex-start;
  padding: 16px;
  border-radius: var(--radius-overlay, 8px);
  background: var(--card-fill);
  border: 1px solid var(--card-stroke);
}
.links {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
  flex: 1;
}
.link {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
code {
  font-family: Consolas, 'Cascadia Mono', monospace;
  font-size: 14px;
  user-select: all;
}
.start {
  justify-content: flex-start;
  margin-top: 4px;
}
.list {
  margin: 8px 0 0;
  padding-left: 20px;
  line-height: 1.7;
}
.note {
  margin: 0;
}
</style>
