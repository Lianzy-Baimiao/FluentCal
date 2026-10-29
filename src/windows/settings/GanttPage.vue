<script setup lang="ts">
import { reactive, ref, watchEffect } from 'vue';
import { confirm, FButton, FColorSwatches, FSelect, FSettingsCard, FSettingsGroup, FTextBox, FToggle, TASK_COLORS } from '@/fluent';
import type { Group } from '@/types';
import { deleteGroup, makeGroup, saveGroup } from '@/lib/tasks';
import { doc, groupList, useSetting } from '@/stores/doc';

const scale = useSetting<string>('gantt.scale');
const showTable = useSetting<boolean>('gantt.showTable');
const showDeps = useSetting<boolean>('gantt.showDeps');
const hideDone = useSetting<boolean>('gantt.hideDone');
const hideSingleDay = useSetting<boolean>('gantt.hideSingleDay');
const colors = TASK_COLORS.map((c) => ({ key: c.key, color: c.bar, label: c.label }));
const count = (id: string) => Object.values(doc.tasks).filter((t) => t.groupId === id).length;

/** Edit buffers for group names, seeded from the store. */
const names = reactive<Record<string, string>>({});
watchEffect(() => {
  for (const g of groupList.value) if (!(g.id in names)) names[g.id] = g.name;
});

const newName = ref('');
async function add() {
  const name = newName.value.trim();
  if (!name) return;
  newName.value = '';
  await saveGroup(makeGroup(name, TASK_COLORS[groupList.value.length % TASK_COLORS.length].key));
}
function rename(g: Group, name: string) {
  const v = name.trim();
  if (v && v !== g.name) void saveGroup({ ...g, name: v });
}
async function remove(g: Group) {
  const n = count(g.id);
  const r = await confirm({ title: `删除分组“${g.name}”？`, content: n ? `组内 ${n} 个任务会移到“未分组”，不会被删除。` : undefined, primaryText: '删除', danger: true });
  if (r === 'primary') await deleteGroup(g.id);
}
</script>

<template>
  <div class="page">
    <FSettingsGroup title="显示">
      <FSettingsCard icon="zoomIn" title="默认时间刻度" description="在甘特图中按住 Ctrl 滚动鼠标滚轮也可以缩放">
        <FSelect v-model="scale" :width="120" :options="[{ value: 'day', label: '日' }, { value: 'week', label: '周' }, { value: 'month', label: '月' }]" />
      </FSettingsCard>
      <FSettingsCard icon="list" title="显示任务表"><FToggle v-model="showTable" /></FSettingsCard>
      <FSettingsCard icon="link" title="显示依赖连线" description="红色虚线表示后续任务开始得比前置任务结束更早">
        <FToggle v-model="showDeps" />
      </FSettingsCard>
      <FSettingsCard icon="completed" title="隐藏已完成的任务"><FToggle v-model="hideDone" /></FSettingsCard>
      <FSettingsCard icon="filter" title="隐藏未分组的单日任务" description="日常待办不出现在甘特图里；里程碑和有依赖的任务不受影响">
        <FToggle v-model="hideSingleDay" />
      </FSettingsCard>
    </FSettingsGroup>
    <FSettingsGroup title="分组">
      <FSettingsCard v-for="g in groupList" :key="g.id" :title="g.name" :description="`${count(g.id)} 个任务`">
        <div class="row-ctl">
          <FTextBox v-model="names[g.id]" :placeholder="g.name" :width="160" @blur="rename(g, names[g.id] ?? '')" @enter="rename(g, names[g.id] ?? '')" />
          <FColorSwatches :model-value="g.color" :colors="colors" :size="18" @update:model-value="(c: string) => saveGroup({ ...g, color: c })" />
          <FButton variant="subtle" size="icon" icon="delete" :title="`删除分组 ${g.name}`" @click="remove(g)" />
        </div>
      </FSettingsCard>
      <FSettingsCard icon="add" title="新建分组">
        <div class="row-ctl">
          <FTextBox v-model="newName" placeholder="分组名称" :width="200" @enter="add" />
          <FButton variant="accent" :disabled="!newName.trim()" @click="add">添加</FButton>
        </div>
      </FSettingsCard>
    </FSettingsGroup>
  </div>
</template>
