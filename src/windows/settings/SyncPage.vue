<script setup lang="ts">
import { computed, inject, onMounted, ref } from 'vue';
import { confirm, FButton, FInfoBar, FSelect, FSettingsCard, FSettingsExpander, FSettingsGroup, FSettingsRow, FTextBox } from '@/fluent';
import { call, type SyncProbe, type SyncStatus } from '@/api';
import { doc, patchSettings, taskList, useSetting } from '@/stores/doc';
import { syncNow, syncStatus, watchSync } from '@/composables/sync';
import { statusLine, uploadedBy } from '@/lib/syncText';
import type { SyncProvider } from '@/types';

watchSync();
const nav = inject<(key: string) => void>('settingsNav');
const s = computed(() => doc.settings.sync);
const provider = useSetting<SyncProvider>('sync.provider');
const cloudProvider = computed(() => ['webdav', 'github', 'gitee', 'gdrive'].includes(provider.value));
/** Turn sync off before changing where it goes. */
const locked = computed(() => s.value.enabled && cloudProvider.value);
const providers = [
  { value: '', label: '不同步' },
  { value: 'webdav', label: 'WebDAV（坚果云、Nextcloud、群晖…）' },
  { value: 'github', label: 'GitHub 私有仓库' },
  { value: 'gitee', label: 'Gitee 私有仓库' },
  { value: 'gdrive', label: 'Google 云端硬盘' },
  { value: 'folder', label: '同步盘文件夹（OneDrive、Dropbox…）' },
];

const davUrl = useSetting<string>('sync.webdav.url');
const davUser = useSetting<string>('sync.webdav.user');
const davFolder = useSetting<string>('sync.webdav.folder');
const clientId = useSetting<string>('sync.gdrive.clientId');
const interval = useSetting<number>('sync.interval');
const proxy = useSetting<string>('sync.proxy');
const gitKey = computed(() => (provider.value === 'gitee' ? 'gitee' : 'github'));
const gitField = (f: 'repo' | 'branch' | 'path') =>
  computed({
    get: () => s.value[gitKey.value][f],
    set: (v: string) => patchSettings({ sync: { [gitKey.value]: { [f]: v } } }),
  });
const repo = gitField('repo');
const branch = gitField('branch');
const path = gitField('path');

// Passwords and tokens: typed here, saved to Windows Credential Manager, never shown again.
const saved = ref<string[]>([]);
const secretInput = ref('');
const googleSecret = ref('');
const secretKey = computed(() => (provider.value === 'webdav' ? 'webdav' : gitKey.value));
const loadSecrets = async () => (saved.value = (await call<string[]>('sync_secrets')) ?? []);
async function saveSecret(key: string, value: string) {
  if (!value.trim()) return;
  try {
    await call('sync_set_secret', { key, value });
    await loadSecrets();
    say('success', '已保存到 Windows 凭据管理器');
  } catch (e) {
    say('error', String(e));
  }
}
async function saveTyped() {
  await saveSecret(secretKey.value, secretInput.value);
  secretInput.value = '';
}
async function saveGoogleSecret() {
  await saveSecret('google-secret', googleSecret.value);
  googleSecret.value = '';
}
const secretHint = (key: string) => (saved.value.includes(key) ? '已保存（输入新值可替换）' : '');

const msg = ref<{ sev: 'success' | 'error' | 'info' | 'warning'; text: string } | null>(null);
const say = (sev: 'success' | 'error' | 'info' | 'warning', text: string) => (msg.value = { sev, text });
const busy = ref(false);

function describe(p: SyncProbe): string {
  if (!p.exists) return `连接成功。云端还没有数据，开启后会上传本机的 ${taskList.value.length} 个任务。`;
  const by = uploadedBy(p.device, p.savedAt);
  return `连接成功。云端已有 ${p.tasks} 个任务、${p.groups} 个分组${by ? `，${by}` : ''}。`;
}

async function probe(): Promise<SyncProbe | null> {
  busy.value = true;
  try {
    return await call<SyncProbe>('sync_probe');
  } catch (e) {
    say('error', `连接失败：${e}`);
    return null;
  } finally {
    busy.value = false;
  }
}

async function test() {
  const p = await probe();
  if (p) say('success', describe(p));
}

async function enable() {
  const p = await probe();
  if (!p) return;
  let replace = false;
  if (p.exists && p.tasks > 0 && taskList.value.length > 0) {
    const by = uploadedBy(p.device, p.savedAt);
    const r = await confirm({
      title: '本机和云端都有数据',
      content: `云端：${p.tasks} 个任务${by ? `（${by}）` : ''}\n本机：${taskList.value.length} 个任务\n\n“合并”保留两边的任务，同一个任务取最后修改的一方。\n“用云端数据”会替换本机的任务（适合刚装好的新电脑）。\n两种方式都会先备份本机数据。`,
      primaryText: '合并',
      secondaryText: '用云端数据',
    });
    if (r === 'close') return;
    replace = r === 'secondary';
  }
  busy.value = true;
  try {
    const st = await call<SyncStatus>('sync_enable', { replace });
    if (st) Object.assign(syncStatus, st);
    say('success', replace ? '已开启同步，本机任务已换成云端数据。' : '已开启同步。');
  } catch (e) {
    say('error', `开启失败：${e}`);
  } finally {
    busy.value = false;
  }
}

async function disable() {
  await call('sync_disable');
  say('info', '已关闭同步。本机数据保留，云端文件也不会删除。');
}

async function googleLogin() {
  busy.value = true;
  say('info', '已在浏览器中打开 Google 登录页面，完成后回到这里。');
  try {
    const email = await call<string | null>('sync_google_login');
    await loadSecrets();
    say('success', `已登录 ${email ?? 'Google 账号'}`);
  } catch (e) {
    say('error', String(e));
  } finally {
    busy.value = false;
  }
}

async function googleLogout() {
  await call('sync_google_logout');
  await loadSecrets();
}

const PROXIES = [
  { value: 'system', label: '使用系统代理' },
  { value: 'direct', label: '不使用代理' },
  { value: 'custom', label: '手动填写…' },
];
const proxyMode = computed({
  get: () => (proxy.value === '' ? 'system' : proxy.value === 'direct' ? 'direct' : 'custom'),
  set: (v: string) => (proxy.value = v === 'system' ? '' : v === 'direct' ? 'direct' : 'http://127.0.0.1:7890'),
});
const INTERVALS = [1, 5, 15, 30, 60].map((m) => ({ value: m, label: `每 ${m} 分钟` })).concat([{ value: 0, label: '只在启动和修改后' }]);
const open = (url: string) => void call('open_url', { url });

onMounted(loadSecrets);
</script>
<template>
  <div class="page">
    <FSettingsGroup title="云同步">
      <FSettingsCard icon="cloud" title="同步状态" :description="statusLine(syncStatus)">
        <div v-if="syncStatus.enabled" class="row-ctl">
          <FButton :disabled="syncStatus.syncing" @click="syncNow">{{ syncStatus.syncing ? '正在同步…' : '立即同步' }}</FButton>
          <FButton variant="subtle" @click="disable">关闭同步</FButton>
        </div>
      </FSettingsCard>
      <FSettingsCard icon="sync" title="同步方式" description="多台电脑使用同一个地方即可共用任务。只同步任务、分组和日期颜色，设置留在每台电脑上">
        <FSelect v-model="provider" :options="providers" :width="290" :disabled="locked" label="同步方式" />
      </FSettingsCard>
      <FInfoBar v-if="locked" severity="info">同步已开启。要换同步方式或地址，请先关闭同步。</FInfoBar>
    </FSettingsGroup>

    <FSettingsGroup v-if="provider === 'webdav'" title="WebDAV">
      <FSettingsCard title="服务器地址" description="坚果云为 https://dav.jianguoyun.com/dav/">
        <div class="row-ctl">
          <FButton size="small" variant="subtle" :disabled="locked" @click="davUrl = 'https://dav.jianguoyun.com/dav/'">坚果云</FButton>
          <FTextBox v-model="davUrl" :width="300" :disabled="locked" placeholder="https://" label="服务器地址" type="url" />
        </div>
      </FSettingsCard>
      <FSettingsCard title="用户名"><FTextBox v-model="davUser" :width="300" :disabled="locked" label="用户名" /></FSettingsCard>
      <FSettingsCard title="密码" :description="secretHint('webdav') || '坚果云请用“第三方应用密码”'">
        <FTextBox v-model="secretInput" type="password" :width="300" :placeholder="saved.includes('webdav') ? '••••••••' : ''" label="密码" @enter="saveTyped" @blur="saveTyped" />
      </FSettingsCard>
      <FSettingsCard title="文件夹" description="数据保存为 文件夹/fluentcal-data.json，不存在会自动创建">
        <FTextBox v-model="davFolder" :width="300" :disabled="locked" label="文件夹" />
      </FSettingsCard>
    </FSettingsGroup>

    <FSettingsGroup v-if="provider === 'github' || provider === 'gitee'" :title="provider === 'github' ? 'GitHub' : 'Gitee'">
      <FSettingsCard title="仓库" description="用户名/仓库名，必须是私有仓库">
        <FTextBox v-model="repo" :width="300" :disabled="locked" placeholder="name/fluentcal-data" label="仓库" />
      </FSettingsCard>
      <FSettingsCard title="Token" :description="secretHint(secretKey) || (provider === 'github' ? '需要这个仓库的 Contents 读写权限' : '需要 projects 权限')">
        <FTextBox v-model="secretInput" type="password" :width="300" :placeholder="saved.includes(secretKey) ? '••••••••' : ''" label="Token" @enter="saveTyped" @blur="saveTyped" />
      </FSettingsCard>
      <FSettingsCard title="分支" description="留空使用默认分支"><FTextBox v-model="branch" :width="300" :disabled="locked" label="分支" /></FSettingsCard>
      <FSettingsCard title="文件路径" description="留空为 fluentcal-data.json"><FTextBox v-model="path" :width="300" :disabled="locked" label="文件路径" /></FSettingsCard>
    </FSettingsGroup>

    <FSettingsGroup v-if="provider === 'gdrive'" title="Google 云端硬盘">
      <FSettingsCard title="OAuth 客户端 ID" description="在 Google Cloud 控制台创建，类型选“桌面应用”（见下方帮助）">
        <FTextBox v-model="clientId" :width="300" :disabled="locked" placeholder="….apps.googleusercontent.com" label="OAuth 客户端 ID" />
      </FSettingsCard>
      <FSettingsCard title="客户端密钥" :description="secretHint('google-secret')">
        <FTextBox v-model="googleSecret" type="password" :width="300" :placeholder="saved.includes('google-secret') ? '••••••••' : ''" label="客户端密钥" @enter="saveGoogleSecret" @blur="saveGoogleSecret" />
      </FSettingsCard>
      <FSettingsCard title="Google 账号" :description="s.gdrive.email ? `已登录 ${s.gdrive.email}` : saved.includes('google-refresh') ? '已登录' : '未登录'">
        <FButton v-if="saved.includes('google-refresh')" @click="googleLogout">退出登录</FButton>
        <FButton v-else variant="accent" :disabled="busy || !clientId || !saved.includes('google-secret')" @click="googleLogin">在浏览器中登录</FButton>
      </FSettingsCard>
    </FSettingsGroup>

    <FSettingsGroup v-if="provider === 'folder'" title="同步盘文件夹">
      <FSettingsCard icon="folder" title="把数据文件夹放到同步盘" description="在“数据”页把数据文件夹改到 OneDrive、Dropbox 或 Google Drive 桌面版的文件夹里，由它们负责同步。各电脑同时修改时按任务合并">
        <FButton @click="nav?.('data')">去设置</FButton>
      </FSettingsCard>
    </FSettingsGroup>
    <template v-if="cloudProvider">
      <div class="row-ctl actions">
        <FButton :disabled="busy" @click="test">测试连接</FButton>
        <FButton v-if="!s.enabled" variant="accent" :disabled="busy" @click="enable">开启同步</FButton>
      </div>
      <FInfoBar v-if="msg" :severity="msg.sev" closable @close="msg = null">{{ msg.text }}</FInfoBar>
      <FInfoBar v-else-if="syncStatus.enabled && syncStatus.error" :severity="syncStatus.halted ? 'error' : 'warning'" :title="syncStatus.halted ? '同步已暂停' : '同步失败'">
        {{ syncStatus.error }}{{ syncStatus.halted ? '。修改设置或点“立即同步”后重试。' : '' }}
      </FInfoBar>

      <FSettingsGroup title="帮助">
        <FSettingsExpander v-if="provider === 'webdav'" icon="help" title="怎样获取坚果云的 WebDAV 密码">
          <template #items>
            <FSettingsRow title="坚果云网页版 › 右上角账户名 › 账户信息 › 安全选项 › 第三方应用管理 › 添加应用，生成的密码填到上面（不是登录密码）。用户名是坚果云的登录邮箱。" />
          </template>
        </FSettingsExpander>
        <FSettingsExpander v-if="provider === 'github'" icon="help" title="怎样创建 GitHub 仓库和 Token">
          <template #items>
            <FSettingsRow title="1. 新建一个私有仓库（Private），例如 fluentcal-data" />
            <FSettingsRow title="2. Settings › Developer settings › Fine-grained tokens › Generate new token：只选这个仓库，Repository permissions 里 Contents 选 Read and write">
              <FButton size="small" variant="subtle" icon="link" @click="open('https://github.com/settings/personal-access-tokens/new')">打开</FButton>
            </FSettingsRow>
            <FSettingsRow title="每次上传是一次提交，可以在仓库里查看历史版本。修改后等 1 分钟左右再上传，减少提交次数。" />
          </template>
        </FSettingsExpander>
        <FSettingsExpander v-if="provider === 'gitee'" icon="help" title="怎样创建 Gitee 仓库和令牌">
          <template #items>
            <FSettingsRow title="1. 新建一个私有仓库" />
            <FSettingsRow title="2. 设置 › 私人令牌 › 生成新令牌，勾选 projects">
              <FButton size="small" variant="subtle" icon="link" @click="open('https://gitee.com/profile/personal_access_tokens')">打开</FButton>
            </FSettingsRow>
          </template>
        </FSettingsExpander>
        <FSettingsExpander v-if="provider === 'gdrive'" icon="help" title="怎样创建 Google OAuth 客户端">
          <template #items>
            <FSettingsRow title="1. 在 Google Cloud 控制台新建项目，启用 Google Drive API">
              <FButton size="small" variant="subtle" icon="link" @click="open('https://console.cloud.google.com/apis/library/drive.googleapis.com')">打开</FButton>
            </FSettingsRow>
            <FSettingsRow title="2. OAuth 同意屏幕：用户类型选“外部”，然后“发布应用”。保持“测试”状态的话，登录 7 天后就会失效" />
            <FSettingsRow title="3. 凭据 › 创建凭据 › OAuth 客户端 ID，应用类型选“桌面应用”，把客户端 ID 和密钥填到上面">
              <FButton size="small" variant="subtle" icon="link" @click="open('https://console.cloud.google.com/apis/credentials')">打开</FButton>
            </FSettingsRow>
            <FSettingsRow title="多台电脑要用同一个客户端：FluentCal 只申请“只能访问本程序创建的文件”的权限，不同客户端互相看不到对方的文件。在国内访问 Google 需要代理（见下方“网络”）。" />
          </template>
        </FSettingsExpander>
      </FSettingsGroup>

      <FSettingsGroup title="高级">
        <FSettingsCard icon="clock" title="自动同步" description="修改后会自动上传；这里设置多久检查一次其他电脑的修改">
          <FSelect v-model="interval" :options="INTERVALS" :width="200" label="自动同步" />
        </FSettingsCard>
        <FSettingsCard icon="globe" title="网络代理" description="局域网地址（NAS 等）总是直连">
          <div class="row-ctl">
            <FTextBox v-if="proxyMode === 'custom'" v-model="proxy" :width="200" placeholder="http://127.0.0.1:7890" label="代理地址" />
            <FSelect v-model="proxyMode" :options="PROXIES" :width="160" label="网络代理" />
          </div>
        </FSettingsCard>
      </FSettingsGroup>
      <p class="note">密码、Token 只保存在本机的 Windows 凭据管理器里，不会写进数据文件、备份或导出文件。开启同步前、用云端数据替换前都会自动备份本机数据（数据 › 备份文件）。</p>
    </template>
  </div>
</template>
