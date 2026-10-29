<script setup lang="ts">
import { FColorSwatches, FNumberBox, FSelect, FSettingsCard, FSettingsGroup, FToggle, TASK_COLORS } from '@/fluent';
import { WEEKDAY_SHORT } from '@/lib/date';
import { doc, patchSettings, useSetting } from '@/stores/doc';

const view = useSetting<string>('calendar.view');
const weekStart = useSetting<number>('calendar.weekStart');
const monthRows = useSetting<string>('calendar.monthRows');
const weeksRows = useSetting<number>('calendar.weeksRows');
const weeksPast = useSetting<number>('calendar.weeksPast');
const showLunar = useSetting<boolean>('calendar.showLunar');
const showHolidays = useSetting<boolean>('calendar.showHolidays');
const showWeekNumbers = useSetting<boolean>('calendar.showWeekNumbers');
const showAlmanac = useSetting<boolean>('calendar.showAlmanac');
const holidayCountdown = useSetting<boolean>('calendar.holidayCountdown');
const showCompleted = useSetting<boolean>('tasks.showCompleted');
const completedLast = useSetting<boolean>('tasks.completedLast');
const autoPostpone = useSetting<boolean>('tasks.autoPostpone');
const defaultColor = useSetting<string>('tasks.defaultColor');
const colors = TASK_COLORS.map((c) => ({ key: c.key, color: c.bar, label: c.label }));

function toggleRest(d: number) {
  const set = new Set(doc.settings.calendar.restDays);
  if (set.has(d)) set.delete(d);
  else set.add(d);
  patchSettings({ calendar: { restDays: [...set].sort() } });
}
const order = [1, 2, 3, 4, 5, 6, 0];
</script>

<template>
  <div class="page">
    <FSettingsGroup title="视图">
      <FSettingsCard icon="calendar" title="默认视图">
        <FSelect v-model="view" :width="150" :options="[{ value: 'month', label: '月视图' }, { value: 'weeks', label: '多周视图' }, { value: 'gantt', label: '甘特图' }]" />
      </FSettingsCard>
      <FSettingsCard icon="calendarWeek" title="每周的第一天">
        <FSelect v-model="weekStart" :width="150" :options="[{ value: 1, label: '星期一' }, { value: 0, label: '星期日' }]" />
      </FSettingsCard>
      <FSettingsCard icon="calendarDay" title="月视图行数" description="自动：只显示本月所在的 4～6 周；固定：总是 6 周">
        <FSelect v-model="monthRows" :width="150" :options="[{ value: 'auto', label: '自动' }, { value: 'six', label: '固定 6 行' }]" />
      </FSettingsCard>
      <FSettingsCard icon="list" title="多周视图" description="显示的周数，以及其中已经过去的周数；日历每周自动向后滚动">
        <div class="row-ctl">
          <span class="note">显示</span><FNumberBox v-model="weeksRows" :min="2" :max="10" suffix="周" :width="96" />
          <span class="note">已过</span><FNumberBox v-model="weeksPast" :min="0" :max="Math.max(0, weeksRows - 1)" suffix="周" :width="96" />
        </div>
      </FSettingsCard>
    </FSettingsGroup>
    <FSettingsGroup title="显示">
      <FSettingsCard icon="moon" title="农历" description="在日期旁显示农历日；节日、节气始终显示">
        <FToggle v-model="showLunar" />
      </FSettingsCard>
      <FSettingsCard icon="flag" title="法定节假日" description="放假显示“休”，调休上班显示“班”">
        <FToggle v-model="showHolidays" />
      </FSettingsCard>
      <FSettingsCard icon="tag" title="周数" description="在每行左侧显示 ISO 周数">
        <FToggle v-model="showWeekNumbers" />
      </FSettingsCard>
      <FSettingsCard icon="info" title="宜忌" description="鼠标停在日期上时显示农历宜忌">
        <FToggle v-model="showAlmanac" />
      </FSettingsCard>
      <FSettingsCard icon="flag" title="节假日倒计时" description="在日历底部显示距离下一个法定假日的天数；任务可在右键菜单或编辑窗口中设为倒数日">
        <FToggle v-model="holidayCountdown" />
      </FSettingsCard>
      <FSettingsCard icon="home" title="休息日" description="没有法定节假日时，这几天按休息日显示">
        <div class="chips" role="group" aria-label="休息日">
          <button v-for="d in order" :key="d" type="button" :class="{ on: doc.settings.calendar.restDays.includes(d) }" :aria-pressed="doc.settings.calendar.restDays.includes(d)" @click="toggleRest(d)">
            {{ WEEKDAY_SHORT[d] }}
          </button>
        </div>
      </FSettingsCard>
    </FSettingsGroup>
    <FSettingsGroup title="任务">
      <FSettingsCard icon="completed" title="显示已完成的任务">
        <FToggle v-model="showCompleted" />
      </FSettingsCard>
      <FSettingsCard icon="list" title="已完成的任务排在最后">
        <FToggle v-model="completedLast" />
      </FSettingsCard>
      <FSettingsCard icon="forward" title="未完成自动顺延" description="过期 30 天内未完成的单日任务，每天自动移到今天（可在任务上单独设置）">
        <FToggle v-model="autoPostpone" />
      </FSettingsCard>
      <FSettingsCard icon="color" title="新任务默认颜色">
        <FColorSwatches v-model="defaultColor" :colors="colors" allow-none :size="22" />
      </FSettingsCard>
    </FSettingsGroup>
  </div>
</template>
