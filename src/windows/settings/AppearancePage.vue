<script setup lang="ts">
import { computed } from 'vue';
import { FColorSwatches, FInfoBar, FSelect, FSettingsCard, FSettingsExpander, FSettingsGroup, FSettingsRow, FSlider, FToggle, WIN_ACCENTS } from '@/fluent';
import { doc, useSetting } from '@/stores/doc';
import { sys } from '@/composables/theme';
import ThemeCards from './ThemeCards.vue';

const followAccent = computed({
  get: () => doc.settings.theme.accent === 'system',
  set: (v: boolean) => (accent.value = v ? 'system' : (sys.info?.accent ?? '#0078D4')),
});
const accent = useSetting<string>('theme.accent');
const accentHex = computed({ get: () => (accent.value === 'system' ? '' : accent.value), set: (v: string) => (accent.value = v || 'system') });
const accents = WIN_ACCENTS.map((c) => ({ key: `#${c}`, color: `#${c}` }));

/** Color settings where '' in the swatch means "automatic". */
const autoColor = (path: string, auto: string) => {
  const s = useSetting<string>(path);
  return computed({ get: () => (s.value === auto ? '' : s.value), set: (v: string) => (s.value = v || auto) });
};
const bgColor = autoColor('desk.bgColor', 'auto');
const todayBorderColor = useSetting<string>('desk.todayBorderColor');
const todayBorderWidth = useSetting<number>('desk.todayBorderWidth');
const todayFillColor = useSetting<string>('desk.todayFillColor');
const restColor = autoColor('desk.restBgColor', 'auto');
const holidayColor = autoColor('desk.holidayTextColor', 'auto');
const termColor = autoColor('desk.termTextColor', 'auto');
const bgSwatches = ['#FFFFFF', '#F3F3F3', '#E8EEF7', '#FFF4E5', '#202020', '#1C2433', '#2B1F2F', '#10241B'].map((c) => ({ key: c, color: c }));
const detailSwatches = ['#C42B1C', '#E74856', '#FF8C00', '#107C10', '#0078D4', '#8764B8', '#E3008C', '#767676'].map((c) => ({ key: c, color: c }));

const bgOpacity = useSetting<number>('desk.bgOpacity');
const cellOpacity = useSetting<number>('desk.cellOpacity');
const acrylic = useSetting<boolean>('desk.acrylic');
const radius = useSetting<number>('desk.radius');
const shadow = useSetting<boolean>('desk.shadow');
const textShadow = useSetting<string>('desk.textShadow');
const fontFamily = useSetting<SettingsFontFamily>('desk.fontFamily');
const textColor = useSetting<string>('desk.textColor');
const taskTextMode = useSetting<'calendar' | 'task'>('desk.taskTextMode');
const textOutlineWidth = useSetting<number>('desk.textOutlineWidth');
const fontScale = useSetting<number>('desk.fontScale');
const cellGap = useSetting<number>('desk.cellGap');
const lineSpacing = useSetting<number>('desk.lineSpacing');
const reduceMotion = useSetting<boolean>('theme.reduceMotion');
const pct = (v: number) => `${v}%`;
type SettingsFontFamily = 'auto' | 'segoe-variable' | 'segoe' | 'yahei-ui' | 'yahei' | 'arial';
const fontOptions = [
  { value: 'auto', label: '系统默认' },
  { value: 'segoe-variable', label: 'Segoe UI Variable' },
  { value: 'segoe', label: 'Segoe UI' },
  { value: 'yahei-ui', label: 'Microsoft YaHei UI' },
  { value: 'yahei', label: 'Microsoft YaHei' },
  { value: 'arial', label: 'Arial' },
];
const textSwatches = ['#000000', '#FFFFFF', '#202020', '#303030', '#B00020', '#006400', '#003E92'].map((color) => ({ key: color, color }));
const textColorMode = computed({
  get: () => textColor.value === 'auto' ? 'auto' : 'custom',
  set: (value: string) => { textColor.value = value === 'auto' ? 'auto' : '#202020'; },
});
</script>

<template>
  <div class="page">
    <FSettingsGroup title="颜色模式">
      <ThemeCards kind="mode" />
    </FSettingsGroup>
    <FSettingsGroup title="强调色">
      <FSettingsExpander icon="color" title="跟随 Windows 强调色" description="今日高亮、按钮、选中状态使用的颜色" :expanded="!followAccent">
        <FToggle v-model="followAccent" />
        <template #items>
          <FSettingsRow title="选择强调色">
            <FColorSwatches v-model="accentHex" :colors="accents" custom :size="28" class="accent-grid" />
          </FSettingsRow>
        </template>
      </FSettingsExpander>
    </FSettingsGroup>
    <FSettingsGroup title="预设方案">
      <ThemeCards kind="preset" />
    </FSettingsGroup>
    <FSettingsGroup title="桌面日历">
      <FSettingsCard icon="palette" title="背景颜色" description="留空则跟随浅色/深色主题">
        <FColorSwatches v-model="bgColor" :colors="bgSwatches" allow-none custom :size="24" />
      </FSettingsCard>
      <FSettingsCard icon="contrast" title="背景不透明度" description="0% 为完全透明，只显示日期格">
        <FSlider v-model="bgOpacity" :min="0" :max="100" :step="1" :width="200" show-value :format="pct" />
      </FSettingsCard>
      <FSettingsCard icon="calendarDay" title="日期格不透明度">
        <FSlider v-model="cellOpacity" :min="0" :max="100" :step="1" :width="200" show-value :format="pct" />
      </FSettingsCard>
      <FSettingsCard icon="window" title="亚克力毛玻璃" description="模糊日历后面的桌面壁纸（Windows 11 效果）">
        <FToggle v-model="acrylic" />
      </FSettingsCard>
      <FInfoBar v-if="acrylic && sys.info && !sys.info.transparency" severity="warning">系统已关闭“透明效果”（设置 › 个性化 › 颜色），亚克力将不会生效。</FInfoBar>
      <FSettingsExpander icon="font" title="布局与文字" description="圆角、阴影、字号与间距">
        <template #items>
          <FSettingsRow title="圆角"><FSlider v-model="radius" :min="0" :max="16" :width="180" show-value :format="(v: number) => `${v}px`" /></FSettingsRow>
          <FSettingsRow title="窗口阴影"><FToggle v-model="shadow" /></FSettingsRow>
          <FSettingsRow title="阴影轮廓" description="自动模式会在透明背景上启用；开关可手动覆盖">
            <FSelect v-model="textShadow" :width="120" :options="[{ value: 'auto', label: '自动' }, { value: 'on', label: '开启' }, { value: 'off', label: '关闭' }]" />
          </FSettingsRow>
          <FSettingsRow title="日历字体"><FSelect v-model="fontFamily" :width="180" :options="fontOptions" /></FSettingsRow>
          <FSettingsRow title="文字颜色" description="自适应根据日期格背景选择黑色或白色">
            <div class="text-color-controls">
              <FSegmented v-model="textColorMode" :options="[{ value: 'auto', label: '自适应' }, { value: 'custom', label: '自选颜色' }]" size="small" />
              <FColorSwatches v-if="textColorMode === 'custom'" v-model="textColor" :colors="textSwatches" custom :size="22" />
            </div>
          </FSettingsRow>
          <FSettingsRow title="任务标题文字颜色" description="跟随日历文字提高可读性；任务/分组颜色仍显示在完成标记上">
            <FSegmented v-model="taskTextMode" :options="[{ value: 'calendar', label: '日历文字' }, { value: 'task', label: '任务颜色' }]" size="small" />
          </FSettingsRow>
          <FSettingsRow title="文字轮廓像素"><FSlider v-model="textOutlineWidth" :min="0" :max="4" :step="0.5" :width="180" show-value :format="(v: number) => `${v}px`" /></FSettingsRow>
          <FSettingsRow title="字号"><FSlider v-model="fontScale" :min="80" :max="140" :step="5" :width="180" show-value :format="pct" /></FSettingsRow>
          <FSettingsRow title="日期格间隙"><FSlider v-model="cellGap" :min="0" :max="10" :width="180" show-value :format="(v: number) => `${v}px`" /></FSettingsRow>
          <FSettingsRow title="任务行距"><FSlider v-model="lineSpacing" :min="0" :max="8" :width="180" show-value :format="(v: number) => `${v}px`" /></FSettingsRow>
        </template>
      </FSettingsExpander>
      <FSettingsExpander icon="color" title="细节颜色" description="今日边框与填充可同时显示，并分别设置 RGBA 颜色">
        <template #items>
          <FSettingsRow title="今日高亮-边框"><FColorSwatches v-model="todayBorderColor" :colors="detailSwatches" allow-none custom :size="22" /></FSettingsRow>
          <FSettingsRow title="今日高亮-边框粗细"><FSlider v-model="todayBorderWidth" :min="0" :max="10" :step="0.5" :width="180" show-value :format="(v: number) => `${v}px`" /></FSettingsRow>
          <FSettingsRow title="今日高亮-填充"><FColorSwatches v-model="todayFillColor" :colors="detailSwatches" allow-none custom :size="22" /></FSettingsRow>
          <FSettingsRow title="休息日背景"><FColorSwatches v-model="restColor" :colors="detailSwatches" allow-none custom :size="22" /></FSettingsRow>
          <FSettingsRow title="节假日文字"><FColorSwatches v-model="holidayColor" :colors="detailSwatches" allow-none custom :size="22" /></FSettingsRow>
          <FSettingsRow title="节日与节气文字"><FColorSwatches v-model="termColor" :colors="detailSwatches" allow-none custom :size="22" /></FSettingsRow>
        </template>
      </FSettingsExpander>
    </FSettingsGroup>
    <FSettingsGroup title="动画">
      <FSettingsCard icon="eye" title="减少动画" description="系统开启“动画效果”关闭时也会自动减少">
        <FToggle v-model="reduceMotion" />
      </FSettingsCard>
    </FSettingsGroup>
  </div>
</template>

<style scoped>
.accent-grid {
  max-width: 420px;
  flex-wrap: wrap;
}
.text-color-controls { display: flex; align-items: center; justify-content: flex-end; gap: 12px; flex-wrap: wrap; }
</style>
