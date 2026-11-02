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

> FluentCal needs no account, sign-in or credentials. On first start it shows sample tasks and a welcome bar. The calendar is placed on the desktop layer (below other windows): press Win+D or Ctrl+Alt+D to see it, or use the tray icon. Right-click a day or a task for its menu; the "..." button opens views, themes and Settings. Cloud sync (WebDAV/GitHub/Gitee) and the LAN web view are optional and off by default; they are not needed to review the app. Public holiday data is downloaded from fastly.jsdelivr.net (falls back to raw.githubusercontent.com).

### 年龄分级（IARC）

应用类型：其他所有应用类型（日历、效率工具）。没有用户间的内容交流或共享、位置共享、应用内购买、浏览器或搜索功能、有年龄限制的内容；预期结果 3+。

## 提交记录
