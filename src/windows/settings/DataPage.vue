<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { confirm, FButton, FInfoBar, FSettingsCard, FSettingsExpander, FSettingsGroup, FSettingsRow } from '@/fluent';
import { call, type DataDirInfo } from '@/api';
import { doc, exportDoc, put, replaceDoc, taskList } from '@/stores/doc';
import { sys } from '@/composables/theme';
import { parseBackup, planMerge, toCSV, toICS } from '@/lib/export';
import { today } from '@/lib/date';

interface Backup { name: string; size: number; modified: number }
const msg = ref<{ sev: 'success' | 'error' | 'info'; text: string } | null>(null);
const backups = ref<Backup[]>([]);
const say = (sev: 'success' | 'error' | 'info', text: string) => (msg.value = { sev, text });
const refresh = async () => (backups.value = await call<Backup[]>('backups_list'));
const dir = ref<DataDirInfo | null>(null);
onMounted(async () => {
  void refresh();
  dir.value = await call<DataDirInfo>('data_dir_info');
});

async function changeDir(reset = false) {
  const path = reset ? null : await call<string | null>('pick_folder');
  if (!reset && !path) return;
  const r = await confirm({
    title: reset ? '恢复默认数据文件夹？' : '更改数据文件夹？',
    content: reset
      ? '数据会合并回本机的默认文件夹，然后程序重新启动。'
      : `数据将保存到：${path}

放在 OneDrive 等同步盘里可以多台电脑共用；文件夹里已有的 FluentCal 数据会与本机数据合并（按每个任务的最后修改时间），不会被覆盖。设置（外观、位置等）仍保存在每台电脑上。程序会重新启动。`,
    primaryText: reset ? '恢复并重启' : '更改并重启',
  });
  if (r !== 'primary') return;
  try {
    await call('data_dir_set', { path });
  } catch (e) {
    say('error', `更改失败：${e}`);
  }
}

async function save(name: string, filter: string, ext: string, content: string, bom = false) {
  try {
    const path = await call<string | null>('file_save', { defaultName: `${name}-${today()}.${ext}`, filterName: filter, extensions: [ext], content, bom });
    if (path) say('success', `已导出到 ${path}`);
  } catch (e) {
    say('error', `导出失败：${e}`);
  }
}
const exportJSON = () => save('FluentCal-备份', 'FluentCal 备份', 'json', JSON.stringify(exportDoc(), null, 2));
const exportCSV = () => save('FluentCal-任务', 'CSV（Excel 可打开）', 'csv', toCSV(taskList.value, doc.groups), true);
const exportICS = () => save('FluentCal-日历', 'iCalendar 日历', 'ics', toICS(taskList.value, doc.groups));

async function importJSON() {
  const text = await call<string | null>('file_open', { filterName: 'FluentCal 备份', extensions: ['json'] }).catch((e) => (say('error', String(e)), null));
  if (!text) return;
  const d = parseBackup(text);
  if (typeof d === 'string') return say('error', d);
  const n = Object.keys(d.tasks).length;
  const plan = planMerge(doc, d);
  const r = await confirm({
    title: '导入备份',
    content: `文件中有 ${n} 个任务。合并：加入 ${plan.added} 个新任务，${plan.updated} 个在文件中更新过的任务用文件版本，其余 ${plan.kept} 个保留本机版本。替换：用文件内容覆盖全部任务。导入前会自动备份当前数据。`,
    primaryText: '合并',
    secondaryText: '替换',
  });
  if (r === 'close') return;
  try {
    await call('backup_create');
  } catch (e) {
    return say('error', `导入前备份失败，已取消导入：${e}`);
  }
  if (r === 'primary') {
    if (Object.keys(plan.groups).length) await put('groups', plan.groups);
    if (Object.keys(plan.tasks).length) await put('tasks', plan.tasks);
    if (Object.keys(plan.dayMarks).length) await put('dayMarks', plan.dayMarks);
    say('success', `已合并：新增 ${plan.added} 个，更新 ${plan.updated} 个，保留本机 ${plan.kept} 个。`);
  } else if (r === 'secondary') {
    await replaceDoc({ ...d, settings: exportDoc().settings, holidays: doc.holidays });
    say('success', `已用备份替换，共 ${n} 个任务。`);
  }
  void refresh();
}

async function backupNow() {
  try {
    say('success', `已创建备份 ${await call<string>('backup_create')}`);
    void refresh();
  } catch (e) {
    say('error', `备份失败：${e}`);
  }
}
async function restore(b: Backup) {
  const r = await confirm({ title: '从备份恢复？', content: `将用 ${b.name} 替换当前全部数据。当前数据会先自动备份。`, primaryText: '恢复', danger: true });
  if (r !== 'primary') return;
  try {
    await call('backup_restore', { name: b.name });
    say('success', '已恢复。');
    void refresh();
  } catch (e) {
    say('error', `恢复失败：${e}`);
  }
}
const kb = (n: number) => `${Math.max(1, Math.round(n / 1024))} KB`;
const when = (ms: number) => new Date(ms).toLocaleString('zh-CN');
</script>

<template>
  <div class="page">
    <FInfoBar v-if="msg" :severity="msg.sev" closable @close="msg = null">{{ msg.text }}</FInfoBar>
    <FSettingsGroup title="导出">
      <FSettingsCard icon="save" title="完整备份（JSON）" description="包含任务、分组、日期颜色和设置，可在其他电脑上导入"><FButton @click="exportJSON">导出</FButton></FSettingsCard>
      <FSettingsCard icon="list" title="任务列表（CSV）" description="用 Excel 打开查看或统计"><FButton @click="exportCSV">导出</FButton></FSettingsCard>
      <FSettingsCard icon="calendar" title="日历文件（ICS）" description="导入到 Outlook、Google 日历或手机日历"><FButton @click="exportICS">导出</FButton></FSettingsCard>
    </FSettingsGroup>
    <FSettingsGroup title="导入">
      <FSettingsCard icon="openFile" title="从备份文件导入" description="选择之前导出的 JSON 备份"><FButton @click="importJSON">选择文件…</FButton></FSettingsCard>
    </FSettingsGroup>
    <FSettingsGroup title="自动备份">
      <FSettingsExpander icon="sync" title="备份文件" :description="`每天自动备份一次，保留最近 14 天。共 ${backups.length} 个备份`">
        <FButton @click="backupNow">立即备份</FButton>
        <template #items>
          <FSettingsRow v-for="b in backups.slice(0, 30)" :key="b.name" :title="b.name" :description="`${when(b.modified)} · ${kb(b.size)}`">
            <FButton size="small" @click="restore(b)">恢复</FButton>
          </FSettingsRow>
          <FSettingsRow v-if="!backups.length" title="还没有备份" />
        </template>
      </FSettingsExpander>
      <FSettingsCard icon="folder" title="数据文件夹" :description="`${dir?.path ?? sys.info?.dataDir ?? ''}${dir?.custom ? '（自定义，可放在同步盘多机共用）' : ''}`">
        <div class="row-ctl">
          <FButton @click="call('open_data_dir')">打开</FButton>
          <FButton @click="changeDir()">更改…</FButton>
          <FButton v-if="dir?.custom" variant="subtle" @click="changeDir(true)">恢复默认</FButton>
        </div>
      </FSettingsCard>
      <p class="note">想通过坚果云、GitHub、Google 云端硬盘等在多台电脑间同步，请到“同步”页设置。</p>
      <FInfoBar v-if="dir?.missing" severity="warning" title="找不到自定义数据文件夹">
        {{ dir.missing }} 现在无法访问（同步盘未启动或磁盘未连接），本次使用的是本机默认文件夹。重新连接后重启程序即可。
      </FInfoBar>
    </FSettingsGroup>
  </div>
</template>
