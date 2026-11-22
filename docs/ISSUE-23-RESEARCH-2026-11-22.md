# FluentCal #23：列表视图中的不定日待办方案

日期：2026-11-22。代码基线：`6a4d38ac5c547c4a03f3a67be65132389a9f0fc7`。状态：方案已按用户选择实施，等待本机验收。

## 结论

Issue [#23「PC 桌面端，列表视图显示缺少」](https://github.com/Lianzy-Baimiao/FluentCal/issues/23) 是设计提案，不是现有功能的 Bug。当前“所有”视图把普通任务放在开始日期；不定日任务没有所属日期，因此刻意不放进日期清单。不能仅把它们塞到创建日期或开始月份，否则会让列表暗示错误日期，勾选完成时也会产生不明确的完成日期。

#23 想探索的是：能否让“所有”视图也查看“本月待办”和“无日期待办”，并在完成后把记录放在用户明确知道的日期下。网页查看是只读的，主要是展示一致性；需要决定完成日期的交互只影响桌面端可操作的列表。

建议不要给每一天都加“显示不定待办”按钮。那会在每个日期标题重复出现，容易让用户以为待办被复制了多份，也会增加列表噪声。优先考虑在“所有”日期清单前放一个独立的“不定日待办”区域，保留“本月待办（不定日）”和“无日期待办”分组；日期清单继续按真实日期排列。

完成日期需要明确表达。采用让“今天”日期标题显示完整日期，并在两个不定日区说明“在桌面勾选时按今天记录”。勾选后任务进入对应日期清单并显示完成状态；回填或改正日期继续使用行菜单里的“修改完成日期…”。网页端保持只读，只展示区块和日期语义。

## 交互备选

| 方案 | 行为 | 主要取舍 |
|---|---|---|
| 独立不定日区（建议） | “所有”顶部展示不定日项目；勾选按当前日期完成，并显示当前完整日期 | 简洁、与左侧待办栏一致；需把“今天”标得足够清楚 |
| 勾选时确认日期 | 点击不定日项目后打开日期选择器，确认实际完成日 | 日期最明确；每次完成多一次操作 |
| 每个日期提供展开入口 | 日期标题有按钮，按需展开该日可完成的不定日项目 | 可以指定日期，但按钮重复、任务容易看成重复项，历史日期也会很多 |

已按第一种方案实施：不定日区置于“所有”列表最前，日期清单只呈现按真实日期完成的项目；每个日期不重复添加待办入口。

## 数据与呈现语义

- 若采用建议方案，`flexSide(today)` 是不定日区的来源。当前月部分只包括开始月份不晚于当前月份的月任务，且含当前月已完成状态；无日期部分只含尚未完成的任务。可单独评估是否把“上月逾期”也并入列表，Issue 当前明确点名的是“本月待办”和“无日期待办”。
- 日期部分只加入已完成槽位：每个 `flexDone` 日期生成已完成 occurrence，开放槽位不制造日期。月槽完成后下月重新开放；历史 `flexDone` 在“所有”里仍按其原日期列出。普通任务、重复任务的既有“所有”语义不变。
- 每月任务在不定日区与完成日期出现两次，是有意的：不定日区显示本月槽位状态，日期项记录发生日期。若关闭“显示已完成”，两处都隐藏已完成项；开放的月任务仍显示。无日期完成后从未完成区消失，并在实际完成日期留下记录。
- “近期”继续只呈现所选日期范围内的 occurrence，不增加不定日区；超出范围的历史完成记录只在“所有”中查看。
- 不定日区的勾选按所选交互写入明确日期；日期清单里的完成项则能按该日期取消完成。沿用现有 `set_occurrence_done`/`clear_day` 行为，避免把 flex 槽位误当成真实日期传给日期计算。

## 边界、风险与验证

需覆盖：尚未到开始月份的月任务不应进入本月部分；跨年月份；本月已完成、无日期已完成及历史完成；同一任务多个月份的完成记录；分组筛选和隐藏已完成；不定日区完成时日期明确可见；当天不再因顶部新增分区而滚动错位。若数据里不同槽位意外记录在同一天，日期列表应沿用现有 occurrence 规则按任务/日期去重，取消该日期时清除该日关联槽位。

原生列表固定行高并按可见区域虚拟化；加入分区标题后，更新 `today_row` 和行种类处理，保持标题、任务项、空行高度一致。当前定位逻辑会在任务被搜索定位时单独向顶部插入 flex 任务 fallback 行（`vm::list_rows`）；新分区需要接管或去重此 fallback，防止同一任务重复放在 `start` 日期。网页上的同一 flex 任务可能同时有首部项和多条日期项，列表 key、搜索定位和高亮要使用任务加 occurrence 身份，不能只取首个相同任务 ID，否则搜索到任务时可能跳到首部而非目标完成日期。特殊首部的无日期项也不应复用会计算截止日/“逾期天数”的普通日期行构造逻辑。

建议测试：

1. Rust `agenda::all_tasks` 与 TypeScript `allTasks`：开放 flex 不进入日期行；不同 `flexDone` 日期进入正确日期、标记完成；同日多槽位去重；普通任务和重复任务不变，隐藏已完成时过滤正确。
2. 桌面 `vm::list_rows`：本月、上月逾期、无日期部分的标题与顺序、独立槽位动作、筛选、隐藏完成、槽位点击和完成日点击；确保“今天”仍滚到今天标题且定位 fallback 不重复。
3. 网页列表组件/e2e：相同数据和次序；“所有”含历史完成日期；“近期”日期范围不变；重复任务不因多 occurrence 身份发生额外重复。

## 主要证据

- Issue 需求：[GitHub #23](https://github.com/Lianzy-Baimiao/FluentCal/issues/23)：提议在“所有”视图查看本月不定日待办、无日期待办及按完成日放置的项目；网页端建议显示一致。
- [native/core/src/agenda.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/native/core/src/agenda.rs#L41-L56)：`all_tasks` 将 flex 从日期列表排除。
- [native/app/src/desk/vm.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/native/app/src/desk/vm.rs#L333-L366)：桌面列表按日期行构建，“所有”只消费 `all_tasks`，定位任务时插入 flex fallback；[同文件](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/native/app/src/desk/vm.rs#L407-L438)：侧栏的本月、逾期、无日期分区与槽位状态。
- [native/core/src/flex.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/native/core/src/flex.rs#L1-L10)：字段与槽位语义；[同文件](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/native/core/src/flex.rs#L82-L128)：完成 occurrence 与月/无日期侧栏数据；[native/core/src/recur.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/native/core/src/recur.rs#L162-L167) 与 [native/core/src/tasks.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/native/core/src/tasks.rs#L287-L310)：按完成日期生成 occurrence，并区分槽位和日期取消。
- [src/lib/agenda.ts](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/src/lib/agenda.ts#L30-L45)：网页“所有”按原始 `start` 建普通日期项，未特别处理 flex；[src/lib/recur.ts](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/src/lib/recur.ts#L126-L131)：网页近期 occurrence 会正确使用完成日期；[src/lib/flex.ts](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/src/lib/flex.ts#L64-L83)：网页侧栏投影。这里是展示语义待统一，不等同于数据或日期计算错误。
- [src/windows/web/AgendaView.vue](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/src/windows/web/AgendaView.vue#L20-L40)：网页范围选择、分组/完成过滤及任务定位；[需求文档.md](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/需求文档.md#L94-L98)：列表范围和 CAL-17 的原始不定日待办语义。
- [native/core/tests/agenda.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/native/core/tests/agenda.rs#L45-L64)、[src/lib/agenda.test.ts](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/src/lib/agenda.test.ts#L43-L65)：现有“所有”测试尚未覆盖 flex；[native/core/tests/recur.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/native/core/tests/recur.rs#L188-L234)、[src/lib/flex.test.ts](https://github.com/Lianzy-Baimiao/FluentCal/blob/6a4d38ac5c547c4a03f3a67be65132389a9f0fc7/src/lib/flex.test.ts#L10-L45)：已有槽位、完成日期和侧栏规则测试可扩展。
