<script setup lang="ts">
import { computed, ref } from 'vue';
import { FButton, FInfoBar, FSelect, FSettingsCard, FSettingsGroup, FTimePicker, FToggle } from '@/fluent';
import { call } from '@/api';
import { doc, useSetting } from '@/stores/doc';
import { computeReminders } from '@/lib/reminders';
import { taskList } from '@/stores/doc';
import ReminderEditor from '../editor/ReminderEditor.vue';

const enabled = useSetting<boolean>('reminders.enabled');
const style = useSetting<string>('reminders.style');
const allDayTime = useSetting<string>('reminders.allDayTime');
const offsets = computed({
  get: () => doc.settings.reminders.defaultOffsets,
  set: (v: number[] | undefined) => (defaults.value = v ?? []),
});
const defaults = useSetting<number[]>('reminders.defaultOffsets');
const msg = ref('');
async function test() {
  try {
    await call('notify_test');
    msg.value = '已发送测试通知。如果没有看到，请检查 Windows 设置 › 系统 › 通知。';
  } catch (e) {
    msg.value = `发送失败：${e}`;
  }
}
const upcoming = computed(() => computeReminders(taskList.value, Date.now(), doc.settings.reminders.allDayTime).slice(0, 5));
const fmt = (ms: number) => new Date(ms).toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', weekday: 'short', hour: '2-digit', minute: '2-digit' });
</script>

<template>
  <div class="page">
    <FSettingsGroup title="通知">
      <FSettingsCard icon="alarm" title="任务提醒" description="到点弹出 Windows 通知。程序在托盘运行即可提醒。">
        <FToggle v-model="enabled" />
      </FSettingsCard>
      <FSettingsCard icon="notification" title="提醒方式" description="提醒卡片显示在屏幕右下角，可以“完成”或“稍后提醒”，不会打断正在输入的内容">
        <FSelect v-model="style" :width="200" :options="[{ value: 'both', label: '通知 + 提醒卡片' }, { value: 'popup', label: '只用提醒卡片' }, { value: 'toast', label: '只用 Windows 通知' }]" />
      </FSettingsCard>
      <FSettingsCard icon="clock" title="全天任务的提醒时间" description="没有设置具体时间的任务在这个时间提醒">
        <FTimePicker v-model="allDayTime" :width="120" />
      </FSettingsCard>
      <FSettingsCard icon="add" title="新任务默认提醒" description="新建任务时自动加上这些提醒">
        <ReminderEditor v-model="offsets" :all-day="false" :all-day-time="allDayTime" :enabled="enabled" />
      </FSettingsCard>
      <FSettingsCard icon="notification" title="发送测试通知">
        <FButton @click="test">测试</FButton>
      </FSettingsCard>
      <FInfoBar v-if="msg" severity="info" closable @close="msg = ''">{{ msg }}</FInfoBar>
    </FSettingsGroup>
    <FSettingsGroup title="接下来 48 小时">
      <FSettingsCard v-for="r in upcoming" :key="r.key" icon="clock" :title="r.title" :description="`${fmt(r.atMs)} · ${r.body}`" />
      <p v-if="!upcoming.length" class="note">接下来 48 小时没有提醒。</p>
    </FSettingsGroup>
  </div>
</template>
