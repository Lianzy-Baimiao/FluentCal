# FluentCal #17–#21：价值与实施范围调研

日期：2026-11-21。代码基线：`f15eca58fc53f0adbfd0e3fd605e35eedcf9bc8c`，功能对应桌面 v0.3.18 / Android 0.3.5。

状态：调研建议，尚未实施。逐条重读了五条 issue、相关源码、现有需求，以及微软、W3C、Google Calendar 和所用节假日数据源的官方资料。五条反馈都来自同一位用户，尚无其他用户附议；没有使用统计或额外访谈，因此不能把它们视作全体用户需求。以下优先级按日常操作收益、误操作影响、改动范围与长期维护成本判断；成本是代码阅读后的相对估计，不是工期承诺。

## 结论

值得改进，但不建议把五条原文全部照做。优先做 #17 的安全默认值和分组信息统一、#21 的定位体验、#19 的可选分组标签；#18 先做有限的内容选择；#20 做基础色盘，通用 Alpha 支持另行安排。

| Issue | 价值判断 | 建议范围 | 相对成本 |
|---|---|---|---|
| [#17 分组完善](https://github.com/Lianzy-Baimiao/FluentCal/issues/17) | 高，值得优先做 | 默认保留组内任务；统一颜色、名称、数量和管理入口，保留不同场景的操作语义 | 默认值小；完整界面统一中 |
| [#21 搜索/倒计时定位](https://github.com/Lianzy-Baimiao/FluentCal/issues/21) | 高，值得优先做 | 显著的短时定位提示、多日范围、连续跳转替换、保持列表视图 | 中；仅改动画小，但不能解决完整需求 |
| [#19 列表分组标签](https://github.com/Lianzy-Baimiao/FluentCal/issues/19) | 中高，值得作为可选显示 | 名称前增加紧凑的分组文字标签，使用分组色，标题优先 | 小到中 |
| [#18 倒计时内容选择](https://github.com/Lianzy-Baimiao/FluentCal/issues/18) | 有价值，建议缩范围分期 | 第一阶段：手工倒数日、选定分组、现有假期来源；普通节日和节气后续加入 | 完整要求中到大 |
| [#20 自定义颜色](https://github.com/Lianzy-Baimiao/FluentCal/issues/20) | 基础色盘值得；全域透明色暂缓 | 色盘、颜色数值、预览、确认/取消；先保持不透明颜色存储 | 色盘中；跨端 Alpha 大 |

这些改进都能设计为由操作或数据变化触发，不需要新增常驻轮询。是否达成“空闲开销不变”仍需实现后的测量；调研没有新增功能的性能数据。

## #17：做一致的信息和入口，不做五个完全相同的弹窗

**当前事实。** 删除分组对话框的 `Picks::new` 设置 `with_tasks: true`，核心删除逻辑已经同时支持“删组并删任务”和“只删组、任务归入未分组”，并接入撤销。因此改为默认不勾选可以复用现有行为。原需求文档 TSK-04 本来就要求默认一起删除，这属于产品默认值调整，而非此前实现偏离需求。[C1]

现有日历/列表任务右键复用同一个菜单构造函数，分组部分只有名称和选中状态。编辑器也是名称列表；设置页、删除框已经有部分颜色/数量信息；主日历筛选浮窗已有颜色、名称、数量、未分组和管理入口。因此统一值得做，很多数据和交互基础可以复用。[C2]

**建议采纳。**

- 删除分组默认保留任务。只有用户主动勾选时才连任务一起删除，确认文案明确显示任务数量，保持一步撤销。
- 右键选择分组、编辑器选分组都补颜色和任务数，并提供“管理分组”；从编辑器进入管理后返回，保留未保存的任务草稿。
- 数量统一为“此组的任务条目总数”，重复任务算一个条目，不把发生次数混入数量；包括已完成任务。这样不同视图、筛选和月份不会让同一个分组的数量含义变化。
- 单选归组、多选筛选、批量删除保留各自行为；共享分组名称、颜色、数量的生成规则。
- “管理分组”应能直接到达对应页面，不再要求用户先理解它为什么藏在“甘特图”设置里。

**建议不按原样采纳。**

- 删除框没有必要加入一个永远不能选的“未分组”伪删除项。用“保留的任务将移到未分组”的说明更直接。若要显示未分组数量，可作为说明，不混入可删除列表。
- “未分组可自定义颜色”暂缓。当前未分组由没有有效 `groupId` 表示，并非真实分组；为了着色而创建一个伪组，会影响默认归组、删除、过滤、导入和同步。若后续做，应保留其语义，用单独的显示设置表达。[C2]

微软对对话框明确要求安全的退出选项和清晰的动作含义；它并没有规定这里的复选框必须如何默认。默认保留任务是结合 FluentCal 的“分类”用途和已有保留路径作出的产品判断。[S1]

## #21：非常值得，核心是“找到任务后立刻看见它”

**当前事实。** 当前并非完全没有高亮：`desk_flash` 保留单日期状态 1600ms，界面显示一条渐隐描边。它没有持续闪烁、底色或多日范围。`desk_jump` 明确把列表/甘特图切成月视图；搜索结果本来带任务 ID，但回调只传日期，倒计时界面也只传日期。因此完整改进要保留目标身份，不能只把 1600 改为 3000。[C3]

**建议采纳。**

- 点击搜索结果或倒计时后，目标出现浅底色加描边，保持约 2–3 秒再淡出；颜色应与“今天”和拖拽目标区分。
- 对多日任务标记当前视口内的日期范围，并突出相应任务；无需自动翻完跨越的所有月份。
- 新的跳转立即替换上一个提示，复用单个可取消的短时状态。
- 列表内搜索保持列表，滚到目标行并提示；日期列表的“近期”只有未来 30–180 天，目标在范围外时需要明确切换到“所有”或临时显示目标，不能假装滚动成功。
- 搜索覆盖全部任务，而视图可能被分组或“隐藏已完成”过滤。目标被隐藏时，应明确提示并提供显示目标的动作，不能只闪日期格却让任务继续不可见。
- 尊重现有“减少动画”：关闭运动效果时仍保留静态短时标记，避免把定位信息一起取消。

**不建议连续快速闪烁。** 需求真正要解决的是定位困难；清晰、短时的标记即可。W3C 关于交互动画的资料支持减少非必要运动和尊重用户偏好；这是设计参考，不是在宣称本机 Slint 界面违反网页标准。[S2]

## #19：值得做，但应保护任务标题的空间

**当前事实。** 列表已经在附加文字中显示分组名，但没有任务名称前的彩色文字标签。它是可发现性改进，不是新增一种分组关系。[C4]

**建议范围。** 增加一个“列表显示分组标签”开关，默认关闭以保持原有密度；开启后对有分组的任务显示紧凑标签。颜色取分组自身颜色，任务原有颜色仍保持独立。未分组默认不加标签，不依赖 #17 的未分组自定义颜色。

长分组名限制宽度并省略，悬停提供全名；窄窗口优先保证任务标题，不新增一整列，也不为每行常驻测量服务。标签必须保留文字，不能只靠颜色识别。Google Calendar 将日历名称和颜色同时作为分类管理手段，W3C 也明确建议颜色之外保留文字或形状信息；这些支持信息表达方式，但不是用户一定需要标签的统计证据。[S3][S4]

## #18：值得做简版，完整版本是“新增内容来源”

**当前事实。** 当前倒计时来自手工标记的任务与“下一个法定假期”，按时间排序；桌面模型最多取六项。它并不显示所有节日。若官方假期数据尚不可得，代码会回退到七个主要节日的日期。原 issue 提议默认勾选“普通节日＋手标倒数日”，与旧行为不等价，升级时照搬会改变用户看到的内容。[C5]

“只选生日分组时，把未手工标记倒数日的生日任务也显示出来”是在增加候选任务，不只是从现有六项中筛选。菜单建议叫“倒计时显示内容”，并与主日历的临时筛选独立。

**第一阶段值得做。**

- 可选择“手工标记的倒数日”“选定分组”“下一个法定假期”。已选来源取并集，同一个任务的同一次发生只出现一次。
- 升级默认保留旧效果，并迁移已有的假期显示开关；不主动把普通节日、节气塞进横条。
- 将选择记为本机显示偏好；全部取消表示没有内容，不能复用主日历“空数组等于全部”的语义。没有匹配结果时，也必须能重新打开内容设置。
- 继续按日期排序，保持短横条；内容太多用手动展开，不增加自动轮播。

**普通节日与节气放第二阶段。** 现有农历模块已有节日、节气与法定安排，但还需要定义候选范围、同日去重、假期起点和数据缺失提示。例如“春节日期”和“春节假期开始”可能不是同一天，不能仅因同名就合并。不同节日恰好同一天也不能粗暴去重。[C6]

所用 holiday-cn 明确说明数据不包含仅因“与周末连休”而休息的普通周末，并且相邻年份文件可能跨年影响。这为“假期”和“每个周末”分开提供了数据基础；不能由一个节日名称推断当天一定放假，未公布年份也不能把预计节日日期冒充官方放假日期。[S5]

实现时缓存节日候选，按日期、任务、配置、假期数据变化更新，不每秒扫描全年；当前“按天”的倒计时没有秒级刷新价值。

## #20：色盘值得做，通用透明色当前不划算

**基础色盘有实际收益。** 大多数用户不会记十六进制色值，降低饱和度、调整明暗也确实比手工算 RGB 方便。微软标准 ColorPicker 包含色谱、RGB/HSV/HEX，并建议按实际场景简化控制；所以可以实现紧凑的色盘和数值输入，不必做成专业绘图软件的面板。[S6]

建议只做一个共用原生颜色选择器，支持预设、色盘、RGB/HEX、明确标注的一种色相/饱和度/明度模型、前后预览及确定/取消。HSV 的明度与 HSL 的亮度并不相同，不混用字段名。交互中的预览不立即写入保存和同步；确认后一次提交，取消恢复原色。这里借鉴 WinUI 的交互规范，不意味着给 Rust/Slint 引入 WinUI 或 WebView 运行时。

**Alpha 不是加一个滑块就结束。** 当前代码存在这些不同约定：[C7]

- 底层 `Rgba::parse` 能读 `#RRGGBBAA`。
- 输入归一化 `normalize_hex` 只接受 3/6 位，任务颜色也只接受不透明颜色。
- 日历背景会主动丢弃颜色内 Alpha，再应用现有背景透明度；休息日底色固定采用 0.22 透明度。
- TypeScript 的 HEX 校验和转换也主要接受 3/6 位；任务色同步到网页和手机后可能退回默认色，不能只改 Windows。
- 现有文字对比度计算以不透明背景为前提；半透明色需要基于合成后的背景判断。

因此推荐先做不透明色盘，继续保留现有背景/单元格透明度设置。全域 Alpha 等明确有哪些颜色项需要透明后再做，并明确“色值 Alpha”和现有透明度到底相乘还是替代、导出/同步/旧客户端如何处理。现阶段投入显著大于它的普遍收益。

## 建议开发顺序与验收重点

1. **先做 #17 默认保留任务。** 同时改需求文档、确认文案和相关测试；覆盖单组/多组、撤销、其他设备同步与任务依赖。
2. **完成 #21 的定位。** 覆盖单日、多日、跨月、重复任务、连续点击、列表过去/未来目标、被过滤目标、减少动画。
3. **一起做 #17 分组信息统一和 #19 可选标签。** 统一计数规则；覆盖编辑草稿、删除/重命名后的刷新、长名称、多语言、窄窗口与高 DPI。
4. **再做 #18 第一阶段与 #20 不透明色盘。** 两者互不依赖，可分别交付；不把普通节日/节气与跨端 Alpha 作为前三项发版的前置条件。

当前不建议采纳：五处交互强制做成同一种弹窗、删除列表加入不可删除的伪分组、默认展示所有节日/节气、快速持续闪烁、一次性开放所有颜色项的 Alpha。

对性能的约束：分组数量在数据变化/打开相关界面时计算；定位动画结束后停止；色盘仅打开时存在；倒数日不新增秒级定时器。实现后再以发布 EXE 对比空闲 CPU、线程和私有工作集，不用理论推断代替实测。

## 主要证据

源码链接固定到 v0.3.18，避免以后修改导致本次结论失去依据。

- **C1** [group_delete.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/app/src/group_delete.rs)、[tasks.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/core/src/tasks.rs)：默认删除选项与 `delete_groups` 两条行为。
- **C2** [menus_build.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/app/src/desk/menus_build.rs)、[editor/view.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/core/src/editor/view.rs)、[overlays.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/app/src/desk/overlays.rs)：分组菜单、编辑选择及筛选计数。
- **C3** [actions.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/app/src/desk/actions.rs)、[wire.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/app/src/desk/wire.rs)、[desk.slint](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/app/ui/desk.slint)：跳转、目标信息与日期提示。
- **C4** [vm.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/app/src/desk/vm.rs)、[desk_list.slint](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/app/ui/desk_list.slint)：列表范围、分组附加文字与行布局。
- **C5** [countdown.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/core/src/countdown.rs)、[countdown.ts](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/src/lib/countdown.ts)：现有倒数日来源及回退逻辑。
- **C6** [lunar.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/core/src/lunar.rs)、[holidays.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/core/src/holidays.rs)：节日节气、法定安排、缓存与在线更新。
- **C7** [fluent.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/app/src/fluent.rs)、[theme/mod.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/core/src/theme/mod.rs)、[theme/desk.rs](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/native/core/src/theme/desk.rs)、[color.ts](https://github.com/Lianzy-Baimiao/FluentCal/blob/v0.3.18/src/lib/color.ts)：不同颜色路径对 Alpha 的处理。
- **S1** [Microsoft：Dialogs and flyouts](https://learn.microsoft.com/en-us/windows/apps/design/controls/dialogs-and-flyouts/dialogs)：安全退出与明确动作。
- **S2** [W3C：Animation from Interactions](https://www.w3.org/WAI/WCAG22/Understanding/animation-from-interactions.html)：减少非必要交互运动与用户偏好，作为设计参考。
- **S3** [Google Calendar：Create a new calendar](https://support.google.com/calendar/answer/37095?hl=en)：按事件类型分日历，以及编辑名称、颜色。
- **S4** [W3C：Use of Color](https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html)：不要只依靠颜色传达信息。
- **S5** [holiday-cn README](https://github.com/NateScarlet/holiday-cn#readme)：国务院公告来源、`isOffDay`、普通周末与跨年说明。
- **S6** [Microsoft：Color picker](https://learn.microsoft.com/en-us/windows/apps/design/controls/color-picker)：按需简化色盘、确认/取消以及可选 Alpha。

外部来源抓取快照在本机 `E:\AI开发\reports\fluentcal-issue-research-2026-11-21\`。没有修改 issue 状态、发布回复或实现上述功能。
