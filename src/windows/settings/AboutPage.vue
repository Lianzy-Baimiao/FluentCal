<script setup lang="ts">
import { FButton, FSettingsCard, FSettingsGroup } from '@/fluent';
import { call } from '@/api';
import { sys } from '@/composables/theme';

const icon = `${import.meta.env.BASE_URL}icon.png`;
const libs = [
  { name: 'Tauri', desc: '应用框架（MIT / Apache-2.0）', url: 'https://tauri.app' },
  { name: 'Vue', desc: '界面框架（MIT）', url: 'https://vuejs.org' },
  { name: 'tyme4ts', desc: '农历、节气、法定节假日（MIT）', url: 'https://github.com/6tail/tyme4ts' },
  { name: 'holiday-cn', desc: '国务院放假安排数据（MIT）', url: 'https://github.com/NateScarlet/holiday-cn' },
];
</script>

<template>
  <div class="page about">
    <div class="hero">
      <img :src="icon" alt="" width="64" height="64" />
      <div>
        <div class="name">FluentCal 桌面日历</div>
        <div class="note">版本 {{ sys.info?.version ?? '—' }} · Windows 内部版本 {{ sys.info?.build ?? '—' }}</div>
        <div class="note">钉在桌面上的 Windows 11 风格日历与甘特图。数据只保存在本机。</div>
      </div>
    </div>
    <FSettingsGroup title="使用技巧">
      <FSettingsCard icon="help" title="双击日期空白处快速添加任务" description="回车保存后可继续输入下一条，Esc 结束" />
      <FSettingsCard icon="mouse" title="拖动任务改期" description="在月视图中拖到其他日期；在甘特图中拖动任务条、两端或底部圆点调整日期和进度" />
      <FSettingsCard icon="keyboard" title="键盘" description="方向键选择日期，Enter 添加，PageUp/PageDown 翻页，Home 回到今天，Ctrl+F 搜索" />
    </FSettingsGroup>
    <FSettingsGroup title="开源组件">
      <FSettingsCard v-for="l in libs" :key="l.name" :title="l.name" :description="l.desc">
        <FButton variant="subtle" icon="link" @click="call('open_url', { url: l.url })">主页</FButton>
      </FSettingsCard>
    </FSettingsGroup>
  </div>
</template>

<style scoped>
.hero {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 16px;
}
.name {
  font-size: 20px;
  font-weight: 600;
  margin-bottom: 4px;
}
</style>
