# FluentCal 0.3.25 — 桌面窗口恢复与拖动起点修复

- 从托盘唤出桌面日历后，锁定或解锁不再让日历退到其他窗口后面。切换到其他应用后，日历仍按原设置回到桌面层。
- Windows 桌面宿主延迟就绪或重新创建时，已显示的日历会重新挂到桌面之上；主动隐藏的日历不会自行弹出。普通窗口和始终置顶模式不受影响。
- 修正移动窗口和从边缘缩放时传给 Windows 的起始鼠标坐标，支持位于主屏左侧或上方的副屏。这项修复针对起拖位置，不是对持续拖动帧率的承诺。

## Windows x64

- [下载安装器](https://github.com/Lianzy-Baimiao/FluentCal/releases/download/v0.3.25/FluentCal_0.3.25_x64-setup.exe)：覆盖安装，沿用已有的日历数据和设置。
- [下载独立 EXE](https://github.com/Lianzy-Baimiao/FluentCal/releases/download/v0.3.25/FluentCal-Desktop-0.3.25.exe)：无需安装，直接运行。
- `FluentCal_0.3.25.0_x64.msix` 是微软商店提交包，供商店签名，不作为 GitHub 下载附件。

本次不更新 Android 客户端。

## 发版验证

- Rust 工作区 269 项通过、0 失败、8 项默认忽略。另单独通过了原生桌面层级回归、带符号屏幕坐标测试，以及真实 Win32 移动和缩放起始消息检查。
- 前端 237 项通过；类型检查、生产构建、翻译检查及桌面交互、国际化、只读网页浏览器回归通过。
- 最终独立 EXE 在隔离桌面上通过 6 个窗口场景；安装器从 0.3.24 原位升级到 0.3.25 后，托盘解锁检查通过，既有日历数据文件的 SHA-256 不变。MSIX 的身份与上一版相同，内含 EXE 与独立程序逐字节一致。

## SHA-256

| 文件 | SHA-256 |
|---|---|
| `FluentCal_0.3.25_x64-setup.exe` | `b6a9d6f66da6112e3168e5acd6d28b03e4292f06b0b56904a48ef36df1bf4c19` |
| `FluentCal-Desktop-0.3.25.exe` | `58d2e2cd097bc9d65d74e8a858d5cff17303373dcafc2617935e7eb1a0a09ec2` |
| `FluentCal_0.3.25.0_x64.msix` | `fc081465514f143fbd51096726c569b74aef1d8f6ec7393bddfbcee15dce9a22` |
