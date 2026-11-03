# FluentCal 商店提交记录

隐私政策：https://raw.githubusercontent.com/Lianzy-Baimiao/FluentCal/main/docs/PRIVACY.md
支持地址：https://github.com/Lianzy-Baimiao/FluentCal/issues
网站：https://github.com/Lianzy-Baimiao/FluentCal

## 商店资料

- 产品名称：FluentCal 桌面日历（与 MSIX 清单的 DisplayName 一致）
- 分类：效率
- 定价：免费
- 平台：Windows 10 1809（17763）及以上 / Windows 11，x64，仅桌面设备

### 中文简短描述

钉在桌面上的日历和甘特图，Windows 11 Fluent 风格。农历、节气和法定节假日“休/班”，任务和甘特图是同一份数据。离线可用，无需账号，无广告。

### 中文描述

钉在 Windows 桌面上的日历 + 甘特图。

FluentCal 位于所有窗口之下、不占任务栏，按 Win+D 显示桌面时仍然可见；也可以切换为始终置顶或普通窗口，锁定位置、开启鼠标穿透。

日历：月视图和多周滚动视图，显示农历、二十四节气、传统节日和法定节假日的“休/班”（含调休，联网自动更新）。双击日期快速添加任务，勾选完成，拖动改期，右键改颜色或删除；多日任务显示为横条。支持重复任务（每天、每周、每月、每年、工作日、农历等）、倒数日和搜索。

甘特图：与日历同一份数据。日/周/月刻度；拖动任务条改日期、拖两端改起止、拖底部圆点改进度；从任务条右端拖到另一条创建依赖；分组汇总和里程碑；可导出 PNG/SVG 图片。

外观：浅色、深色或跟随系统；跟随 Windows 强调色或自选颜色；背景色、透明度、亚克力、圆角、字号、间距均可调，内置 5 套预设方案。

数据：保存在本机，每天自动备份，可导出 JSON、CSV（Excel）和 ICS（Outlook、Google 日历）。提醒使用 Windows 通知或屏幕右下角的提醒卡片。

可选功能：通过你自己的 WebDAV 网盘或 GitHub/Gitee 私有仓库在多台电脑间同步；在本机浏览器或同一局域网的手机上只读查看日历。

不需要账号，没有广告，不收集使用数据。同步和网页查看默认关闭，只在你设置后才会使用网络。

### 中文功能（产品功能列表）

1. 钉在桌面：在所有窗口之下，Win+D 后仍可见
2. 月视图、多周视图：农历、节气、法定节假日“休/班”
3. 甘特图：拖动改期、依赖连线、分组汇总、里程碑
4. 重复任务、提醒、倒数日、搜索
5. 浅色/深色、强调色、透明度、亚克力、预设方案
6. 本机保存、每日备份，导出 JSON/CSV/ICS
7. 可选：WebDAV/GitHub/Gitee 同步，浏览器只读查看

### 中文关键词

桌面日历；甘特图；农历；日程；待办；节假日；倒数日

### English short description

A calendar and Gantt chart pinned to the Windows desktop, in Windows 11 Fluent style, with the Chinese lunar calendar and public holidays. Works offline, with no account or advertising.

### English description

A calendar and Gantt chart that sit on your Windows desktop.

FluentCal stays below all windows, takes no taskbar space and remains visible after Win+D (Show desktop). You can also make it always on top or an ordinary window, lock its position, or let mouse clicks pass through it.

Calendar: month and scrolling multi-week views with the Chinese lunar calendar, the 24 solar terms, traditional festivals and China's public holidays and make-up work days (updated online). Double-click a day to add a task, tick to complete, drag to reschedule, right-click to recolour or delete. Multi-day tasks show as bars. Repeating tasks (daily, weekly, monthly, yearly, workdays, lunar dates and more), countdowns and search are included.

Gantt chart: the same data as the calendar, with day, week and month scales. Drag bars to move tasks, drag their ends to change dates, drag the handle to set progress, and drag from one bar to another to add a dependency. Group summaries, milestones and PNG/SVG export.

Appearance: light, dark or follow Windows; the Windows accent colour or your own; background colour, transparency, acrylic, corner radius, font size and spacing, with five presets.

Data is stored on your PC with a daily backup, and can be exported as JSON, CSV (Excel) and ICS (Outlook, Google Calendar). Reminders use Windows notifications or a card at the bottom right of the screen.

Optional: sync between PCs through your own WebDAV storage or a private GitHub/Gitee repository, and view the calendar read-only in a browser on this PC or on a phone on the same network.

No account, no advertising, no usage data collected. Sync and the web view are off by default and use the network only after you set them up.

### English features

1. Pinned to the desktop: below all windows, still visible after Win+D
2. Month and multi-week views with the Chinese lunar calendar and public holidays
3. Gantt chart: drag to reschedule, dependencies, group summaries, milestones
4. Repeating tasks, reminders, countdowns and search
5. Light/dark themes, accent colours, transparency, acrylic, presets
6. Stored on your PC with daily backups; export JSON/CSV/ICS
7. Optional sync (WebDAV, GitHub, Gitee) and a read-only web view

### English keywords

desktop calendar; Gantt chart; lunar calendar; schedule; to-do; holidays; countdown

### runFullTrust 用途说明（≤ 500 字符）

> FluentCal is a native Rust Win32 desktop calendar, packaged as a packaged classic app at medium IL. runFullTrust launches the desktop EXE and lets it place its window on the desktop layer (owned by the shell's desktop window, below other windows), register global hotkeys, show a tray icon and notifications, use file dialogs, and read/write the user's own data file. No elevation, services, drivers or background tasks beyond the optional startup task.

### 审核说明（Additional testing info）

> FluentCal needs no account, sign-in, credentials or purchase. The interface is in Simplified Chinese. On first start it shows sample tasks and a welcome bar.
>
> Where the window is: by default the calendar sits on the desktop layer, below other windows. Press Win+D (show desktop) to see it, or press Ctrl+Alt+D or left-click the tray icon to bring it forward. Starting the app again from Start opens its Settings window. The tray menu has 显示日历 (show calendar), 设置… (Settings) and 退出 (quit).
>
> What to try: the header switches between 月 (month), 多周 (multi-week) and 甘特 (Gantt chart). Double-click a day to add a task, right-click a day or a task for its menu, and drag tasks to reschedule. The 更多 (…) button in the header has search, the theme, the window level (desktop layer / always on top / normal window), 外观… (appearance) and 设置… (Settings).
>
> Start with Windows is on by default, through the package's startup task; it takes effect after the first launch. It can be turned off in Settings › 常规 › 开机自动启动 or in Task Manager's Startup tab.
>
> Network: China's public holiday data is downloaded from fastly.jsdelivr.net (falling back to raw.githubusercontent.com); Settings › 常规 › 自动更新法定节假日 turns this off. Cloud sync (WebDAV/GitHub/Gitee) and the LAN web view are optional and off by default; they are not needed to review the app.

### 年龄分级（IARC）

应用类型：其他所有应用类型（日历、效率工具）。没有用户间的内容交流或共享、位置共享、应用内购买、浏览器或搜索功能、有年龄限制的内容；预期结果 3+。

## 提交记录

### 0.3.0.0 首次提交（2026-11-02 18:00 送审，正在认证）

- 产品 **FluentCal 桌面日历 / 9NTT06453DG5**（包族 `ice.FluentCal_rz8m2sa64m4em`），Submission 1 **1152921505702293830**。用户要求“改完打包给我安装包，然后上传微软商店”后提交。
- 包：`FluentCal_0.3.0.0_x64.msix`（9.6 MB，SHA-256 `4c31e0c01f2f4f0c7fd2052506b6af3ca0ee80c55524e70b30674b456312be81`），后台显示 Validated；仅桌面设备，未勾选“由 Microsoft 决定未来设备系列”。包内 `fluentcal.exe` 与同次构建的安装包 `FluentCal_0.3.0_x64-setup.exe`（SHA-256 `45eaf2cd…3c282`）里的相同，含菜单宽度修复（521a48a）。
- 定价和可用性：免费（CNY 0），240 个市场，公开可见。
- 属性：效率类；隐私政策、网站、支持地址见本文开头。
- 年龄分级：IARC 10.3，9 个问题都答“否”，结果 IARC 3+、ESRB 所有人、PEGI 3+、USK 所有人、Microsoft 3+；分级 ID `afe5d54a-952c-86c6-8310-3fb8c4443fff`。IARC 使用条款的确认框留给用户本人勾选，保存后重新打开分级页核对过。
- Store 一览：只有中文（中国），与包的 `Resource Language="zh-CN"` 和纯中文界面一致，没有加英文页。描述、7 项功能、简短描述、7 个关键词、开发者“白描ice”与上文逐字一致；4 张桌面截图依次为 month-light、month-dark、gantt、settings-appearance，按 SHA-256 与 `docs/store/` 对上。重新打开页面核对过。
- 提交选项：发布暂缓为“除非我选择‘立即发布’，否则不发布此提交”；runFullTrust 用途说明 453 字符，重新打开后一致。
- 其他测试信息：上文“审核说明”1329 字符，重新打开后一致；凭据表为空。
- 送审后状态：“正在认证”，页面提示“单击‘立即发布’时，产品将开始发布”。认证通过后要有人在产品概述页点“立即发布”才会上架。商店链接 https://apps.microsoft.com/detail/9NTT06453DG5 上架后才可用。
- runFullTrust 是受限能力，Validated 不代表已获批准，以认证结果为准。
- 操作脚本、截图在忽略的 `dist/store-0.3.0/`；临时 Edge（CDP 127.0.0.1:9441）和它的配置目录已在送审后清理。

### 0.3.1.0 重新送审（2026-11-02 22:40，正在认证）

- 接管时，同一个 Submission 1 已处于草稿状态，包含旧包 `0.3.0.0`、验证失败的 `0.3.0.1` 和已验证的 `0.3.1.0`；错误包导致“保存”和“提交进行认证”不可用。
- 删除失败的 `0.3.0.1` 后，等待后台删除完成再保存。Partner Center 自动移除覆盖同一设备范围的旧包 `0.3.0.0`。保存后的总览只包含 **FluentCal_0.3.1.0_x64.msix / Validated**，所有提交部分均为“完成”。没有再次上传程序包。
- 已点击“提交进行认证”，后台确认 **正在认证 / 预处理正在进行**。`runFullTrust` 用途说明已核对保留。
- 2026-11-02 22:44 按用户要求取消手动发布暂缓，选择“在通过认证后立即发布我的应用”并点击“应用”。后台确认 **产品在通过认证后立即开始发布**。无需再手动点击“立即发布”；当前仍等待认证。设置截图：`release/store-proof/auto-publish-2026-11-02.jpg`。
- 商店包：`release/FluentCal_0.3.1.0_x64.msix`，SHA-256 `d0c1c67100148723d1d331f40acea405f2fd0b8b7d5b5edf7c9468d8fd915a5f`。应用版本仍为 0.3.0，本次只增加商店包版本以区分更新后的内容。
- 独立安装包：`release/FluentCal_0.3.0_x64-setup.exe`，SHA-256 `f0edb4a81a3d4a41b355370e3eb5828d7251cfb864fdcdd68fb59d44b99ba035`。
- 本地商店包内 `fluentcal.exe` 的 SHA-256 为 `9d866fa95eee830151b4fcd61ea8f06203dee1af3da2b3d9c5f6bf09e0eccd29`。校验了 ZIP 完整性，并用修复后的脚本重新构建 `0.3.1.0`，其全部包内文件与 `release/` 中的待发包一致。
- 认证页面截图：`release/store-proof/certification-2026-11-02.jpg`（本地忽略目录）。

#### 避免重复上传和非法版本

- 微软商店保留四段版本号的最后一段，必须为 `0`；替换内容应增加前三段之一，例如 `0.3.0.0 → 0.3.1.0`。`0.3.0.1` 会被商店拒绝，即使 SDK 打包成功。
- `scripts/build-msix.sh` 现在在读取 SDK 和修改打包目录前校验版本格式、第四段和每段的 `0..65535` 范围。回归命令 `bash scripts/test-msix-version.sh` 的 7 个非法版本用例均通过；合法版本实际打包成功。
- 页面出现目标文件后，不要因尚未验证完成而再次选取同一文件。必须按目标文件名和版本观察结果，不能把旧包的 Validated 或旧的 Save 按钮状态当作新包完成。删除时同样等待 Deleting 完成再继续。
- 后续以 [产品概述页](https://partner.microsoft.com/zh-cn/dashboard/products/9NTT06453DG5/overview) 的认证结果为准；本节记录的是送审成功，并非已上架。

### 0.3.2.0 输入修复更新（2026-11-03，待上传）

- 应用版本与 Windows 文件版本更新为 **0.3.2**，商店包为 **0.3.2.0**，高于此前已使用的 `0.3.1.0`。
- 修复调整日历位置并完成调整后，未激活的日历窗口双击日期出现输入框却无法输入的问题。输入框创建时主动请求键盘激活，支持输入、回车保存和连续添加。
- 沿用正式商店身份：`ice.FluentCal`，发布者 `CN=56079E3F-5861-4D9C-B9C3-93F76224C42E`，发布者显示名“白描ice”；产品 ID 为 `9NTT06453DG5`。
- 已构建并通过 MakeAppx 校验：`release/FluentCal_0.3.2.0_x64.msix`，SHA-256 `2d8cdaff20d9b22ac080932b77a1fc0917c1058c918664ed44bb9c5d95256bf2`。
- 独立安装包：`release/FluentCal_0.3.2_x64-setup.exe`，SHA-256 `3177749e5ae7a5a68339ecd79f2a0740bc2672277237de2f162ce5c78e743c44`。两种包使用同次构建的 `fluentcal.exe`，SHA-256 `cfc113f3c06a3ea1b3f3cae8c10d212358ec273e3843aefb0d379d8dc6d70e68`；已读取 MSIX 中的 EXE 校验一致，并确认 EXE 文件版本、产品版本均为 `0.3.2`。
- 验证：网页构建与类型检查、完整原生测试、商店非法版本检查通过；输入修复已通过真实 Windows 焦点回归。
- 提交说明建议：`修复调整日历位置后双击日期无法输入的问题，现在可以正常输入、回车保存并连续添加任务。`
- 本轮操作 Partner Center 时，界面自动化因无法可靠识别 Edge 当前网址而停止。**本次尚未上传或送审**，当前线上及认证状态没有重新确认。后续从产品概述页继续，沿用“通过认证后立即发布”的用户选择。
