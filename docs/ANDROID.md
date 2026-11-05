# FluentCal Android 客户端

Android 客户端（0.3.1）在同一个仓库开发，采用 Vue 3 + Capacitor 8。复用桌面网页端的任务模型、重复规则、农历、甘特计算和备份格式，独立提供适合触屏的界面，并兼容原生桌面版的自动云同步协议。

下载：[GitHub 安卓发布页](https://github.com/Lianzy-Baimiao/FluentCal/releases/tag/android-latest)。新安装或已有 0.3.0 正式版的用户安装 `FluentCal-Android-0.3.1.apk`，可直接覆盖升级并保留数据。

**旧测试版覆盖升级**：本机早期 0.1.0 / 0.2.0 debug 包与正式包签名不同。Android 9 及以上可先安装 `FluentCal-Android-0.3.1-legacy-upgrade.apk`，它包含旧签名授权新签名的 v3 轮换证明，允许保留应用数据迁移；后续直接使用普通正式包。无需先卸载。Android 7/8 不支持签名轮换，应先导出 JSON，再卸载旧 debug 包、安装正式包并导入；不要在 Android 7/8 使用迁移包作为正式通道。签名链与 installed-data 能力已通过 Android SDK apksig 校验，实际设备上的覆盖安装与数据保留尚待真机验收。

0.3.1 同时修复扫码页面横屏问题（手机锁定竖屏），以及底部日程数字角标被按钮行高拉长的问题；角标固定高度，个位数为圆形、多位数为胶囊形，超过 99 显示 `99+`。

## 功能与使用

- **日程**：未来 30 天的安排、搜索全部任务、完成/取消完成、撤销。
- **月历**：农历、节气、节日、任务标记；点日期查看当天安排并创建任务。
- **甘特图**：按日/周刻度、滑动时间轴、分组筛选、进度、里程碑和依赖线；点任务修改起止日期及进度。重复任务显示系列的初始安排。
- **任务编辑**：日期、时间、重复、提醒、颜色、分组、进度、备注；重复任务支持仅删除这一次或删除整个系列。编辑普通字段时保留桌面端的复杂重复规则、排除日期、完成日期、依赖及其他字段。
- **本地数据**：Android 应用私有存储，离线启动和编辑；写入按顺序处理，保留上一份有效快照。浏览器开发使用单独的 `CapacitorStorage.fluentcal.mobile.doc.v1`，不与桌面 mock 数据混用。
- **自动云同步**：WebDAV / GitHub / Gitee；启动、恢复前台、联网、本机编辑后和运行期间定时同步。支持首次合并/使用云端日历、离线补传、删除传播、并发冲突重试、连接测试、状态提示和手动重试。
- **扫码连接**：电脑版「设置 → 同步」显示一次性二维码和 GitHub 下载入口，手机「设置 → 扫码连接电脑」直接获取已配置的云同步账号。二维码有效 5 分钟，获取一次即失效，离开桌面同步页/取消/刷新可撤销。
- **更新检查**：手机「设置 → 移动端更新 → 检查 GitHub 更新」检查独立的 `android-latest` 发布通道。只有有效移动端版本和已上传的 APK 才会提示更新；网络失败、限流或发布未完成会明确报错。下载更新会打开 GitHub 发布页，由用户下载并安装。
- **数据交换**：导入桌面 JSON，先显示新增、更新和保留数量，确认后合并；同一任务取 `updatedAt` 较新的一方，本机外观设置保留。导出 JSON、CSV、ICS，通过系统分享面板保存或发送到其他应用。
- **提醒**：在设置中开启系统通知权限，再给任务选择提醒时间。预排未来 30 天、最多 64 条，打开应用、恢复前台或修改任务后刷新；完成或删除任务时取消旧提醒。全天任务使用 09:00。通知送达会受 Android 电池优化和精确闹钟权限影响。
- **适配**：浅色/深色/跟随系统、手机与平板、横屏、系统返回键、状态栏安全区域、软键盘调整。

第一次启动带有标记清楚的示例项目，日程页和设置页都可以清除，也可以撤销。

## 接入自动云同步

1. 安装桌面版 0.3.7 或更新版本，在电脑的「设置 → 同步」开启 WebDAV，或 GitHub/Gitee 私有仓库同步。
2. 在该页点击「去 GitHub 下载移动端」，将 APK 安装到手机。手机和电脑连接同一局域网。
3. 电脑点击「生成二维码」，手机进入「设置 → 自动云同步 → 扫码连接电脑」，允许相机权限并扫描。账号、服务器和令牌会自动导入，无需逐项输入。获取云端任务摘要后，选择「合并并开启同步」或「使用云端日历」。后者适合手机只有示例项目的情况。
4. 开启后即可在两端编辑。首次同步前会永久保留一份本机原始备份，重试不会覆盖它；设置中可导出该备份，再通过「导入备份」找回任务。

二维码授权手机使用电脑当前的云同步账号，仅供自己的设备扫描。配对服务临时监听一个随机局域网端口，二维码携带随机 256 位 AES 密钥和 128 位会话标识，网络只传输 AES-GCM 密文，密钥不发送到 HTTP 服务；配对信息不写入桌面数据文件或日志。手机只接受 RFC1918 私有 IPv4 的 `/pair/{id}` 一次性端点，不跟随重定向。日常云同步仍使用 HTTPS。配对完毕后手机独立连接云端，不需要电脑一直运行。

连接失败时确认手机与电脑在同一网络、没有 Wi-Fi 客户端隔离；在桌面选择手机可访问的网卡，允许 FluentCal 通过 Windows **专用网络**防火墙，然后重新生成二维码。若不方便扫码，可展开「手动配置」，仍支持原有方式。设备已开启同步时，需先关闭同步才能更换配对电脑。

WebDAV 使用 **HTTPS** 地址，文件名固定为 `fluentcal-data.json`，默认文件夹为 `FluentCal`；支持逐层建目录。更新现有文件要求服务返回强 ETag，用 `If-Match` 防止覆盖同时发生的修改。未提供强 ETag 的服务会显示配置错误，需在服务端开启或改用 GitHub/Gitee。遇到跳转需要填写最终地址，不会将凭据转发到跳转目标。应用使用 Android 系统网络，不读取桌面版代理设置。

GitHub/Gitee 只允许私有仓库；令牌需要对目标仓库文件的读写权限，GitHub fine-grained token 对目标仓库授予 **Contents: Read and write** 即可。空分支使用仓库默认分支，空路径使用 `fluentcal-data.json`。凭据采用 Android Keystore AES-GCM 加密，密文在 `noBackupFilesDir`；不进入 Preferences、导出的 JSON 或云端文件。浏览器开发预览的凭据只保存在当前页面内存，刷新后要重新输入，正式 APK 会安全持久保存。

同步范围为任务、分组、日期标记和相应修改时间。设置、节假日缓存、密码和令牌不上传云端同步文件。与桌面 `native/core/src/merge.rs` 保持相同的逐条合并规则：较新修改胜出，同时间保留本机，删除记录保留 60 天。两台设备需保持系统时间准确；离线超过 60 天的旧副本可能重新带回已过期删除记录。

WebDAV 在连续编辑结束约 2 秒后上传，GitHub/Gitee 约 1 分钟后合并提交。默认每 5 分钟拉取一次，可选 1/5/15/30 分钟或只在事件触发时同步。网络/服务错误会退避重试；认证、配置或损坏云端文件错误会暂停并提示修复，点击「立即同步」重试。遇到不支持的新格式或损坏的云端文件，不会上传覆盖。

定时检查依赖应用进程仍在运行。切到后台时尝试补同步，恢复前台及联网时再同步；**本版本不使用 Android 后台 Worker，系统挂起或结束应用后，不承诺持续定时同步，下次打开自动补同步。**

## 通过文件交换数据

1. 桌面版「设置 → 数据」导出 JSON。
2. 将文件保存到手机，在 Android 客户端「设置 → 导入备份」选择文件。
3. 核对合并摘要后点「确认合并」。日期/重复规则不合法的文件会被拒绝，原数据保留。
4. 手机修改后使用「导出备份」，将 JSON 放回电脑并在桌面端合并导入。

手动备份合并不会传播删除操作，导入旧备份可能重新加入已删除任务；自动云同步会传播删除。桌面小组件及移动端多语言界面尚未实现。桌面「网页查看」仍是现有的只读功能，配对使用独立的短时服务。

## 开发与打包

需要 Node.js 22+、JDK 21、Android SDK 36。当前 Android Gradle Plugin 使用 Build Tools 35.0.0，缺少时会自动下载。手机要求 Android 7.0 / API 24 及以上，并安装较新的 Android System WebView（Chromium 120+）；旧版本会显示升级提示。

```powershell
npm ci
npm run android:dev       # http://localhost:1420，默认打开手机界面
npm run android:build     # TypeScript 检查 + dist-android/
npm run android:sync      # 构建并把资源和插件同步至 android/
npm run android:open      # 在 Android Studio 中打开

$env:JAVA_HOME = 'C:\path\to\jdk-21'
$env:ANDROID_HOME = 'C:\path\to\Android\Sdk'
npm run android:apk      # Windows 一键生成可安装的 debug APK
```

输出：`release/FluentCal-Android-0.3.1-debug.apk`（versionCode 4）。使用 `scripts/build-android.ps1 -Release -PreviousApk release/FluentCal-Android-0.3.0.apk` 构建正式签名包 `release/FluentCal-Android-0.3.1.apk` 并检查与上一正式版的签名兼容性。

构建脚本提供 `.apk.sha256` 校验和，并会自动识别装在 `%USERPROFILE%\tools\fluentcal-android` 下的工具链；在其他机器上按上例设置环境变量。如果 Gradle 需要代理，可通过当前终端的 `JAVA_OPTS` 设置 `-Dhttps.proxyHost=地址 -Dhttps.proxyPort=端口`，不需要改项目配置。Windows 中文路径下 Gradle worker 使用 `file.encoding=COMPAT` 生成与 Java 启动器一致的参数文件，Java 源码编译显式使用 UTF-8。

其他平台可在 `npm run android:sync` 后进入 `android` 运行 `./gradlew :app:assembleDebug`。正式发布需设置 `FLUENTCAL_ANDROID_KEYSTORE`、`FLUENTCAL_ANDROID_STORE_PASSWORD`（alias 为 `fluentcal`），或使用本机工具目录 `signing/` 中的发布密钥与 DPAPI 密码文件。构建脚本自动读取本机凭据、验证签名并清除子进程密码环境变量。发布密钥不进入仓库，应妥善备份；后续更新必须继续使用同一签名。

GitHub 使用稳定标签 `android-latest`，发布名称必须为 `FluentCal Android vX.Y.Z`，APK 资产名称必须为 `FluentCal-Android-X.Y.Z.apk`。这个标签专用于移动端，发布时不设为仓库 latest，Windows 的 `/releases/latest` 下载入口继续指向桌面版。更新脚本只接受该仓库、该标签、已上传 APK 以及递增的三段版本号。

迁移包在普通正式包构建完成后运行 `scripts/build-android-migration.ps1 -PreviousApk release/FluentCal-Android-0.1.0-debug.apk` 生成。必须保留原始 `.android/debug.keystore` 和正式发布密钥；脚本在仓库外保存签名轮换证明，开启旧证书的 installed-data 能力、禁止 rollback，并验证 Android 9/12L/13/16 上「旧测试包 → 迁移包 → 普通正式包」的证书关系。迁移包复用正式包内容，不启用调试；Android 9+ 使用正式签名，Android 7/8 只能识别它的旧 v2 签名，所以迁移入口明确限定 Android 9+。仅在迁移路径上提供此包，自动更新仍检查普通正式包。

桌面版「设置 → 关于 → 检查更新」通过 WinRT `StoreContext.GetAppAndOptionalStorePackageUpdatesAsync` 检查微软商店，找到更新后从「打开 Microsoft Store」交给商店安装。非 MSIX 安装会提示前往商店查看；请求失败不会显示“已是最新”。新增代码不代表已提交或发布新的微软商店版本，商店上架仍走现有提交流程。

`npm run build` 的桌面网页资源仍输出到 `dist/`，移动端输出为 `dist-android/`。原生桌面 Rust/Slint 工程不受移动端构建影响。

## 验证

```powershell
npm test                 # 包括备份校验、并发持久化、写入失败恢复、通知清理测试
npm run android:e2e       # Edge 无头模式，运行手机交互测试并在 shots/android/ 生成截图
npm run android:sync-e2e  # 两个隔离客户端连接模拟 WebDAV，测试真实界面与自动调度
node scripts/android-navigation-e2e.mjs  # 角标 0/1/12/99+，320/390/768px，深浅主题
powershell -File scripts/check-android-package.ps1 -Apk release/FluentCal-Android-0.3.1.apk
npm run build            # 同时检查桌面网页资源可构建
cd android
.\gradlew.bat :app:testDebugUnitTest :app:lintDebug --console=plain
```

浏览器交互测试涵盖：创建并重新加载、完成、放弃编辑、月历创建、甘特编辑、复杂重复规则保留、备份预览/合并/无效文件拒绝、导出、删除撤销、360px 手机/横屏/768px 平板无页面横向溢出、深浅色。它不会替代 Android 真机上的系统通知、系统文件选择器、分享面板及返回手势测试。

本次验证：182 项前端单元测试、8 组移动端交互回归、两个独立客户端的 7 组同步/更新集成测试、5 项 Android 原生网络测试、5 项 Rust 配对协议测试及 8 项桌面设置测试通过；Android lint、正式 APK 编译与 v2 签名校验通过。二维码从 980px 和 760px 桌面窗口的真实渲染图中成功解码，共享加密样本通过 Rust/WebCrypto 双端校验。当前没有连接 Android 设备或真实云账号，相机权限/Keystore 生命周期、真实云服务和正式商店包中的 Store API 仍需设备环境验收。

## 代码入口

| 路径 | 职责 |
|---|---|
| `src/windows/mobile/MobileApp.vue` | 手机导航、日程、月历、设置和数据交换 |
| `src/windows/mobile/MobileEditor.vue` | 任务编辑、删除确认、重复规则保留 |
| `src/windows/mobile/MobileGantt.vue` | 可滑动时间轴、进度和依赖线 |
| `src/mobile/backend.ts` | 原生持久化、串行写入、快照恢复、系统分享 |
| `src/mobile/data.ts` | 导入和恢复前的数据校验 |
| `src/mobile/notifications.ts` | 原生本地通知调度与取消 |
| `src/mobile/pairing.ts`、`native/core/src/pairing.rs` | 一次性二维码、局域网加密配置交接 |
| `src/mobile/updates.ts`、`src/windows/mobile/MobileUpdates.vue` | GitHub 移动端更新检查 |
| `native/app/src/windows/settings/mobile.rs` | 桌面配对入口、二维码状态和商店检查 |
| `native/app/src/platform/package.rs` | WinRT 微软商店更新接口 |
| `src/windows/mobile/MobileSync.vue` | 手机云同步配置、首次接入、状态与备份入口 |
| `src/mobile/sync-model.ts` | 桌面兼容协议、合并、删除记录与条件写入循环 |
| `src/mobile/sync-remote.ts` | WebDAV / GitHub / Gitee 适配器 |
| `src/mobile/sync.ts` | 启动/编辑/联网/恢复/定时触发和错误退避 |
| `src/mobile/vault.ts`、`android/app/src/main/java/com/fluentcal/mobile/SyncVaultPlugin.java` | Android Keystore 凭据加密 |
| `android/app/src/main/java/com/fluentcal/mobile/SyncHttpClient.java` | HTTPS 原生请求、WebDAV 方法及响应大小限制 |
| `android/`、`capacitor.config.ts` | 原生工程、图标、系统配置 |
| `scripts/build-android.ps1` | Windows APK 构建及校验和 |
