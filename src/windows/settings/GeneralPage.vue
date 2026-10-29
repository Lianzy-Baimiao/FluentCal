<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { FButton, FInfoBar, FSelect, FSettingsCard, FSettingsGroup, FShortcutBox, FToggle } from '@/fluent';
import { call, on, screenTitle, type Screen } from '@/api';
import { doc, useSetting } from '@/stores/doc';
import { updateHolidays } from '@/stores/holidays';
import { sys, refreshSystem } from '@/composables/theme';

const autostart = useSetting<boolean>('general.autostart');
const pinMode = useSetting<string>('desk.pinMode');
const locked = useSetting<boolean>('desk.locked');
const clickThrough = useSetting<boolean>('desk.clickThrough');
const noActivate = useSetting<boolean>('desk.noActivate');
const toggleKey = useSetting<string>('shortcuts.toggle');
const newKey = useSetting<string>('shortcuts.newTask');
const holidayAuto = useSetting<boolean>('holidays.autoUpdate');

const pins = [
  { value: 'desktop', label: '嵌入桌面（推荐）' },
  { value: 'top', label: '始终置顶' },
  { value: 'normal', label: '普通窗口' },
];
const pinHint = computed(
  () =>
    ({
      desktop: '钉在桌面上：在所有窗口之下，按 Win+D 显示桌面时仍可见，不占任务栏。',
      top: '始终显示在其他窗口之上（会挡住其他程序）。',
      normal: '和普通程序一样，可出现在任务栏和 Alt+Tab 中。',
    })[pinMode.value] ?? '',
);

const checking = ref(false);
const holidayMsg = ref('');
const lastCheck = computed(() => (doc.settings.holidays.lastCheck ? new Date(doc.settings.holidays.lastCheck).toLocaleString('zh-CN') : '从未'));
const years = computed(() => Object.keys(doc.holidays).sort().join('、') || '无（使用内置数据）');
async function checkHolidays() {
  checking.value = true;
  const r = await updateHolidays(true);
  checking.value = false;
  holidayMsg.value = r.failed ? '无法连接节假日数据源，请检查网络。' : r.updated.length ? `已更新 ${r.updated.join('、')} 年放假安排。` : '已是最新，暂无新的放假安排。';
}

async function onShortcut() {
  setTimeout(refreshSystem, 300);
}

// Which screen the calendar is on (only shown with more than one screen).
const screens = ref<Screen[]>([]);
const screenOpts = computed(() => screens.value.map((s) => ({ value: s.index, label: screenTitle(s) })));
const screen = computed({
  get: () => screens.value.find((s) => s.current)?.index ?? null,
  set: (index: number | null) => {
    if (index !== null) void call('desk_move_to_screen', { index });
  },
});
const loadScreens = async () => (screens.value = (await call<Screen[]>('screens_list')) ?? []);
let off: (() => void) | undefined;
onMounted(() => {
  void loadScreens();
  off = on('desk://screen', () => void loadScreens());
});
onUnmounted(() => off?.());
</script>

<template>
  <div class="page">
    <FSettingsGroup title="启动">
      <FSettingsCard icon="power" title="开机自动启动" :description="sys.info?.debug ? '开发版本不会写入开机启动项' : '登录 Windows 后自动显示桌面日历'">
        <FToggle v-model="autostart" />
      </FSettingsCard>
    </FSettingsGroup>
    <FSettingsGroup title="桌面日历窗口">
      <FSettingsCard icon="desktop" title="窗口层级" :description="pinHint">
        <FSelect v-model="pinMode" :options="pins" :width="180" />
      </FSettingsCard>
      <FSettingsCard v-if="screens.length > 1" icon="desktop" title="显示在" description="把日历移到另一块屏幕（保持在屏幕上的相对位置）。解锁后也可以直接拖过去。">
        <FSelect v-model="screen" :options="screenOpts" :width="220" />
      </FSettingsCard>
      <FSettingsCard icon="lock" title="锁定位置和大小" description="解锁后按住日历任意空白处拖动来移动，拖动四边或四角调整大小">
        <FToggle v-model="locked" />
      </FSettingsCard>
      <FSettingsCard icon="mouse" title="鼠标穿透" description="日历只显示、不响应鼠标，可以点到下方的桌面图标。在托盘菜单中关闭。">
        <FToggle v-model="clickThrough" />
      </FSettingsCard>
      <FSettingsCard v-if="pinMode === 'desktop'" icon="desktop" title="点击日历时不抢焦点" description="保持 Win+D“显示桌面”正常：点击日历后再按 Win+D 仍能恢复窗口。输入文字时日历会自动获得键盘；快捷键（Ctrl+Z 等）需先用唤出快捷键把日历调到前面。">
        <FToggle v-model="noActivate" />
      </FSettingsCard>
      <FSettingsCard icon="eyeOff" title="隐藏桌面日历" description="隐藏后可从托盘图标或快捷键重新打开">
        <FButton @click="call('desk_hide')">隐藏</FButton>
      </FSettingsCard>
    </FSettingsGroup>
    <FSettingsGroup title="快捷键">
      <FSettingsCard icon="keyboard" title="唤出日历" description="在任意程序中按下，日历临时显示在最前；再按一次或点到别处即回到桌面层">
        <FShortcutBox v-model="toggleKey" @update:model-value="onShortcut" />
      </FSettingsCard>
      <FSettingsCard icon="add" title="快速新建任务">
        <FShortcutBox v-model="newKey" @update:model-value="onShortcut" />
      </FSettingsCard>
      <FInfoBar v-if="sys.info?.shortcutErrors?.length" severity="warning" title="部分快捷键注册失败">
        {{ sys.info.shortcutErrors.join('；') }}。可能已被其他程序占用，请换一个组合。
      </FInfoBar>
    </FSettingsGroup>
    <FSettingsGroup title="节假日">
      <FSettingsCard icon="sync" title="自动更新法定节假日" :description="`联网获取国务院公布的放假和调休安排。上次检查：${lastCheck}；在线数据：${years}`">
        <div class="row-ctl">
          <FButton :disabled="checking" @click="checkHolidays">{{ checking ? '正在检查…' : '立即检查' }}</FButton>
          <FToggle v-model="holidayAuto" />
        </div>
      </FSettingsCard>
      <FInfoBar v-if="holidayMsg" severity="info" closable @close="holidayMsg = ''">{{ holidayMsg }}</FInfoBar>
    </FSettingsGroup>
  </div>
</template>
