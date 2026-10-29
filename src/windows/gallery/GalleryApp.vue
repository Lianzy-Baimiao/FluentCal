<script setup lang="ts">
import { onMounted, ref } from 'vue';
import {
  applyTheme, bestTextOn, confirm, FButton, FCalendarView, FCheckbox, FColorSwatches, FDatePicker, FDialog, FFlyout, FIcon,
  FField, FInfoBar, FNavView, FNumberBox, FProgressBar, FRadioGroup, FSegmented, FSelect, FSettingsCard, FSettingsExpander,
  FSettingsGroup, FSettingsRow, FShortcutBox, FSlider, FTextBox, FTimePicker, FToggle, GLYPHS, showMenu, SVG_ICONS,
  TASK_COLORS, vTip, WIN_ACCENTS, type MenuItem,
} from '@/fluent';

const q = new URLSearchParams(location.search);
const dark = q.get('theme') === 'dark';
applyTheme(document.documentElement, { dark, accent: q.get('accent') ?? '#0078D4' });
document.body.style.background = dark ? '#202020' : '#F3F3F3';

const on = ref(true);
const off = ref(false);
const chk = ref(true);
const chk2 = ref(false);
const task = ref(true);
const radio = ref('dark');
const slider = ref(60);
const select = ref('month');
const text = ref('');
const text2 = ref('写周报');
const notes = ref('多行文本\n第二行');
const num = ref(15);
const date = ref('2026-10-30');
const time = ref('09:30');
const color = ref('blue');
const accentPick = ref('#0078D4');
const seg = ref('month');
const seg2 = ref('day');
const nav = ref('appearance');
const shortcut = ref('Ctrl+Alt+D');
const flyout = ref(false);
const dialog = ref(false);
const expanded = ref(true);
const flyBtn = ref<HTMLElement>();
const menuBtn = ref<HTMLElement>();
const glyphs = Object.entries(GLYPHS);
const svgNames = Object.keys(SVG_ICONS);
const swatchColors = TASK_COLORS.map((c) => ({ key: c.key, color: c.bar, label: c.label }));
const accentColors = WIN_ACCENTS.slice(0, 16).map((c) => ({ key: c, color: c }));

const lunar = (iso: string) => {
  const d = +iso.slice(8);
  return d === 1 ? '九月' : d === 10 ? '寒露' : ['初', '十', '廿'][Math.min(2, Math.floor((d - 1) / 10))] + '一二三四五六七八九十'[(d - 1) % 10];
};
const markers = (iso: string) => (iso >= '2026-10-01' && iso <= '2026-10-07' ? '休' : iso === '2026-10-10' ? '班' : undefined);

const menuItems: MenuItem[] = [
  { label: '新建任务', icon: 'add', shortcut: 'Ctrl+N', action: () => {} },
  { label: '编辑', icon: 'edit', action: () => {} },
  { separator: true },
  { label: '显示已完成', checked: true, action: () => {} },
  { label: '视图', icon: 'calendar', children: [
    { label: '月视图', checked: true, radio: true },
    { label: '多周视图', checked: false, radio: true },
    { label: '甘特图', checked: false, radio: true },
  ] },
  { label: '颜色', swatches: TASK_COLORS.slice(0, 8).map((c, i) => ({ key: c.key, color: c.bar, title: c.label, selected: i === 5 })) },
  { separator: true },
  { label: '禁用项', icon: 'lock', disabled: true },
  { label: '删除', icon: 'delete', danger: true, shortcut: 'Del', action: () => {} },
];

function openMenu() {
  if (menuBtn.value) showMenu(menuItems, menuBtn.value);
}
async function askConfirm() {
  await confirm({ title: '删除任务？', content: '“写周报”将被删除，此操作无法撤销。', primaryText: '删除', danger: true });
}

onMounted(() => {
  const open = q.get('open');
  if (open === 'menu') openMenu();
  else if (open === 'flyout') flyout.value = true;
  else if (open === 'dialog') dialog.value = true;
  else if (open === 'confirm') askConfirm();
});
</script>
<template>
  <div class="gallery">
    <h1 class="t-title">FluentCal 控件库 · {{ dark ? '深色' : '浅色' }}</h1>

    <section>
      <h2 class="t-subtitle">按钮</h2>
      <div class="row">
        <FButton>标准按钮</FButton>
        <FButton variant="accent">强调按钮</FButton>
        <FButton variant="subtle" icon="add">透明按钮</FButton>
        <FButton icon="save">带图标</FButton>
        <FButton icon-right="chevronDown">下拉</FButton>
        <FButton disabled>禁用</FButton>
        <FButton variant="accent" disabled>禁用强调</FButton>
        <FButton size="small">小按钮</FButton>
        <FButton size="icon" icon="settings" title="设置" />
        <FButton size="icon" variant="subtle" icon="pin" title="钉住" :pressed="true" />
        <FButton size="icon" variant="subtle" icon="more" title="更多" />
      </div>
    </section>

    <section>
      <h2 class="t-subtitle">开关 / 复选框 / 单选</h2>
      <div class="row">
        <FToggle v-model="on" />
        <FToggle v-model="off" />
        <FToggle v-model="on" text-position="right" />
        <FToggle :model-value="true" disabled />
        <FToggle :model-value="false" disabled />
      </div>
      <div class="row">
        <FCheckbox v-model="chk" label="已选中" />
        <FCheckbox v-model="chk2" label="未选中" />
        <FCheckbox :model-value="false" indeterminate label="部分选中" />
        <FCheckbox :model-value="true" disabled label="禁用" />
        <FCheckbox v-model="task" round :size="16" aria-label="任务" />
        <FCheckbox :model-value="false" round :size="16" aria-label="任务2" />
        <FCheckbox :model-value="true" round :size="16" color="#D13438" aria-label="红色任务" />
      </div>
      <FRadioGroup
        v-model="radio"
        direction="horizontal"
        :options="[{ value: 'light', label: '浅色' }, { value: 'dark', label: '深色' }, { value: 'system', label: '跟随系统' }, { value: 'x', label: '禁用', disabled: true }]"
      />
    </section>

    <section>
      <h2 class="t-subtitle">输入</h2>
      <div class="row">
        <FTextBox v-model="text" placeholder="搜索任务" icon="search" clearable :width="220" />
        <FTextBox v-model="text2" clearable :width="200" aria-label="任务名称" />
        <FTextBox model-value="禁用文本" disabled :width="160" aria-label="禁用文本框" />
        <FNumberBox v-model="num" :min="0" :max="60" suffix="分钟" :width="140" />
        <FSelect v-model="select" :width="160" :options="[{ value: 'month', label: '月视图' }, { value: 'weeks', label: '多周视图' }, { value: 'gantt', label: '甘特图' }, { value: 'x', label: '不可用', disabled: true }]" />
        <FSelect :model-value="null" placeholder="请选择" :options="[{ value: 1, label: '一' }]" :width="140" />
      </div>
      <FTextBox v-model="notes" multiline :rows="3" :width="420" placeholder="备注" />
      <div class="row">
        <FSlider v-model="slider" :min="0" :max="100" show-value :format="(v) => `${v}%`" />
        <FSlider :model-value="30" disabled />
        <div style="width: 240px"><FProgressBar :value="slider" /></div>
        <div style="width: 160px"><FProgressBar /></div>
      </div>
    </section>

    <section>
      <h2 class="t-subtitle">日期与时间</h2>
      <div class="row top">
        <FDatePicker v-model="date" :lunar="lunar" :markers="markers" />
        <FDatePicker model-value="" clearable />
        <FTimePicker v-model="time" clearable />
        <FShortcutBox v-model="shortcut" />
        <FShortcutBox model-value="" />
      </div>
      <div class="row top">
        <div class="card-surface"><FCalendarView v-model="date" :lunar="lunar" :markers="markers" /></div>
        <div class="card-surface"><FCalendarView v-model="date" :first-day="0" /></div>
      </div>
    </section>

    <section>
      <h2 class="t-subtitle">颜色 / 分段</h2>
      <FColorSwatches v-model="color" :colors="swatchColors" allow-none custom />
      <FColorSwatches v-model="accentPick" :colors="accentColors" :size="32" />
      <div class="row">
        <FSegmented v-model="seg" :options="[{ value: 'month', label: '月', icon: 'calendar' }, { value: 'weeks', label: '多周', icon: 'calendarWeek' }, { value: 'gantt', label: '甘特', icon: 'gantt' }]" />
        <FSegmented v-model="seg2" size="small" :options="[{ value: 'day', label: '日' }, { value: 'week', label: '周' }, { value: 'month', label: '月' }]" />
        <FSegmented v-model="seg" :options="[{ value: 'month', icon: 'calendar', title: '月' }, { value: 'weeks', icon: 'calendarWeek', title: '多周' }, { value: 'gantt', icon: 'gantt', title: '甘特' }]" />
      </div>
    </section>

    <section>
      <h2 class="t-subtitle">信息栏</h2>
      <FInfoBar title="提示" closable>拖动标题栏可移动日历，完成后点击锁定。</FInfoBar>
      <FInfoBar severity="success" title="已保存">数据已备份到本地。</FInfoBar>
      <FInfoBar severity="warning" title="注意">快捷键 Ctrl+Alt+D 已被其他程序占用。
        <template #action><FButton>更改</FButton></template>
      </FInfoBar>
      <FInfoBar severity="error" title="错误" closable>节假日数据更新失败，请检查网络。</FInfoBar>
    </section>

    <section>
      <h2 class="t-subtitle">设置卡片</h2>
      <FSettingsGroup title="桌面">
        <FSettingsCard icon="desktop" title="嵌入桌面" description="日历位于所有窗口之下，显示桌面时仍可见">
          <FToggle v-model="on" />
        </FSettingsCard>
        <FSettingsCard icon="layers" title="窗口层级">
          <FSelect v-model="select" :width="160" :options="[{ value: 'month', label: '嵌入桌面' }, { value: 'weeks', label: '始终置顶' }, { value: 'gantt', label: '普通窗口' }]" />
        </FSettingsCard>
        <FSettingsCard icon="palette" title="主题与颜色" description="浅色、深色、强调色" clickable />
        <FSettingsExpander v-model:expanded="expanded" icon="brightness" title="背景" description="颜色、不透明度与圆角">
          <FButton>恢复默认</FButton>
          <template #items>
            <FSettingsRow title="背景不透明度"><FSlider v-model="slider" show-value :format="(v) => `${v}%`" /></FSettingsRow>
            <FSettingsRow title="亚克力效果" description="模糊桌面壁纸"><FToggle v-model="off" /></FSettingsRow>
            <FSettingsRow title="圆角"><FNumberBox v-model="num" :min="0" :max="20" suffix="px" /></FSettingsRow>
          </template>
        </FSettingsExpander>
        <FSettingsExpander title="无图标展开项" description="没有图标时行左边距对齐">
          <template #items><FSettingsRow title="子项"><FToggle v-model="on" /></FSettingsRow></template>
        </FSettingsExpander>
        <FSettingsCard icon="lock" title="禁用卡片" disabled><FToggle :model-value="false" disabled /></FSettingsCard>
      </FSettingsGroup>
    </section>

    <section>
      <h2 class="t-subtitle">导航</h2>
      <div class="nav-demo">
        <FNavView
          v-model="nav"
          title="FluentCal 设置"
          :items="[{ key: 'general', label: '常规', icon: 'settings' }, { key: 'appearance', label: '外观', icon: 'palette' }, { key: 'calendar', label: '日历', icon: 'calendar' }, { key: 'gantt', label: '甘特图', icon: 'gantt' }]"
          :footer-items="[{ key: 'about', label: '关于', icon: 'info' }]"
        >
          <FSettingsCard icon="brightness" title="选择模式"><FSelect v-model="radio" :width="140" :options="[{ value: 'light', label: '浅色' }, { value: 'dark', label: '深色' }]" /></FSettingsCard>
        </FNavView>
      </div>
    </section>

    <section>
      <h2 class="t-subtitle">浮出层 / 菜单 / 对话框 / 提示</h2>
      <div class="row">
        <span ref="flyBtn"><FButton @click="flyout = !flyout">打开浮出层</FButton></span>
        <span ref="menuBtn"><FButton icon-right="chevronDown" @click="openMenu">菜单</FButton></span>
        <FButton @click="dialog = true">对话框</FButton>
        <FButton @click="askConfirm">确认框</FButton>
        <FButton v-tip="'这是一个工具提示'">悬停提示</FButton>
      </div>
      <FFlyout v-model:open="flyout" :anchor="flyBtn" :width="280" label="示例浮出层">
        <div class="t-body-strong">浮出层标题</div>
        <p class="t-secondary" style="margin: 4px 0 12px">轻触外部或按 Esc 关闭。</p>
        <FButton variant="accent" @click="flyout = false">知道了</FButton>
      </FFlyout>
      <FDialog v-model:open="dialog" title="编辑分组" primary-text="保存" secondary-text="不保存" @primary="dialog = false" @secondary="dialog = false">
        <FField label="分组名称"><FTextBox v-model="text2" width="100%" /></FField>
      </FDialog>
    </section>

    <section>
      <h2 class="t-subtitle">图标（{{ glyphs.length + svgNames.length }}）</h2>
      <div class="icons">
        <div v-for="[n] in glyphs" :key="n" class="ic"><FIcon :name="n" :size="20" /><span>{{ n }}</span></div>
        <div v-for="n in svgNames" :key="n" class="ic svg"><FIcon :name="n" :size="20" /><span>{{ n }}</span></div>
      </div>
    </section>

    <section>
      <h2 class="t-subtitle">任务颜色</h2>
      <div class="row top">
        <div v-for="t in (['light', 'dark'] as const)" :key="t" class="theme-panel" :data-theme="t">
          <div v-for="c in TASK_COLORS" :key="c.key" class="tc">
            <span class="bar" :style="{ background: c.bar, color: bestTextOn(c.bar) }">{{ c.label }}色任务条</span>
            <span :style="{ color: t === 'dark' ? c.textDark : c.textLight }">{{ c.label }} · 文字示例</span>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.gallery { max-width: 1200px; margin: 0 auto; padding: 24px 40px 80px; }
.t-title { margin: 0 0 16px; }
.gallery > section {
  margin: 0 0 16px;
  padding: 16px 20px 20px;
  border-radius: var(--radius-overlay);
  border: 1px solid var(--card-stroke);
  background: var(--card-fill);
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.gallery > section > h2 { margin: 0 0 4px; }
.row { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; }
.row.top { align-items: flex-start; }
.card-surface {
  border-radius: var(--radius-overlay);
  border: 1px solid var(--surface-stroke-flyout);
  background: var(--flyout-fill);
  box-shadow: var(--shadow-flyout);
}
.nav-demo { height: 340px; border-radius: var(--radius-overlay); border: 1px solid var(--card-stroke); background: var(--solid-base); overflow: hidden; }
.icons { display: grid; grid-template-columns: repeat(auto-fill, minmax(112px, 1fr)); gap: 4px; }
.ic { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: var(--radius-control); background: var(--control-alt-fill); font-size: 11px; color: var(--text-secondary); }
.ic :deep(.f-icon) { color: var(--text-primary); }
.ic.svg { outline: 1px dashed var(--accent-fill); }
.theme-panel { flex: 1; min-width: 280px; padding: 12px; border-radius: var(--radius-overlay); display: flex; flex-direction: column; gap: 6px; }
.theme-panel[data-theme='light'] { background: #FFFFFF; color: var(--text-primary); }
.theme-panel[data-theme='dark'] { background: #202020; color: var(--text-primary); }
.tc { display: flex; align-items: center; gap: 12px; }
.bar { width: 120px; padding: 2px 8px; border-radius: 4px; color: #fff; font-size: 12px; line-height: 18px; }
</style>
