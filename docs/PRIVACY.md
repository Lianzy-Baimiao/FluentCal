# FluentCal 隐私政策 / Privacy Policy

生效日期 / Effective date: 2026-11-05

## 中文

FluentCal 桌面日历是钉在 Windows 桌面上的日历和甘特图工具。Microsoft Store 的发布者显示名称为“白描ice”，项目维护地址为 https://github.com/Lianzy-Baimiao/FluentCal 。

### 你的任务和设置

- FluentCal 不要求登录，不包含广告、用户行为分析或自动上传崩溃报告的服务。应用不向开发者发送你的任务、设置、使用记录或设备标识。
- 任务、分组、设置保存在本机的 `data.json`，每天自动备份一份到同一文件夹的 `backups`。独立安装版的文件夹是 `%APPDATA%\com.fluentcal.desktop`；Microsoft Store 版写在该应用包自己的数据目录里。你可以在“设置 › 数据”里把数据文件夹改到别的位置（例如 OneDrive 等同步盘），这时数据由该位置的服务按你的设置处理。
- 导出（JSON、CSV、ICS、甘特图图片）只写到你选择的文件。卸载独立安装版不会删除你的数据；Store 版卸载时 Windows 通常会删除该包的数据。

### 会联网的功能

- **法定节假日更新**（默认开启，可在“设置 › 常规”关闭）：从 fastly.jsdelivr.net 或 raw.githubusercontent.com 下载公开的节假日数据（NateScarlet/holiday-cn）。请求里没有你的任务或设置，但这些服务能看到你的 IP 地址和请求时间。
- **云同步**（默认关闭）：只有你在“设置 › 同步”里填写了自己的 WebDAV 网盘或 GitHub / Gitee 仓库后才会运行。任务数据在你的电脑和你选择的服务之间直接传输，不经过开发者；云端是 JSON 明文，由该服务本身的访问控制保护。密码和令牌保存在 Windows 凭据管理器中，不写进数据文件。
- **网页查看**（默认关闭）：“在浏览器中查看”在本机启动一个只监听 127.0.0.1 的只读网页。打开“允许局域网内的设备查看”后，同一网络中的设备凭访问码可以只读查看你的日历；访问码保存在凭据管理器中，可随时重置或关闭。
- **安卓扫码配对**（由你主动生成）：桌面临时启动局域网配对服务，将当前云同步配置及凭据通过 AES-GCM 加密交给扫描二维码的手机。二维码携带解密密钥，有效 5 分钟，获取一次即失效；取消、刷新或离开同步页会撤销。配对信息不上传开发者服务器，不写入任务数据或日志。请只让自己的设备扫描。
- **Android 客户端**：任务与设置保存在应用私有存储，云同步凭据用 Android Keystore 加密，保存在不参与系统备份的目录中。手机直接连接你选择的云服务；相机权限仅用于扫码，不保存或上传扫码照片。导出文件由你通过系统分享面板选择接收位置或应用；卸载通常会删除本机应用数据。
- **检查更新**：桌面通过 Microsoft Store 查询更新；安卓向 GitHub 查询本项目的公开发布信息。查询不包含任务或云同步凭据，服务商可看到请求的 IP 地址和时间。下载移动端或更新会打开 GitHub 发布页。
- 你主动点击的链接（项目主页、帮助文档等）由浏览器和对应网站按各自的隐私政策处理。

### 系统功能

提醒使用 Windows 通知。开机启动使用 Windows 的启动项（Store 版使用应用的启动任务），可在设置中关闭。Microsoft Store 的账号、分发、更新和诊断由微软按其隐私声明处理，FluentCal 不控制这些服务。

### 联系与变更

隐私问题可通过 https://github.com/Lianzy-Baimiao/FluentCal/issues 联系维护者。GitHub issue 是公开的，请勿提交密码、令牌或不愿公开的任务内容。如今后数据处理方式发生变化，本政策和生效日期会同步更新。

## English

FluentCal is a calendar and Gantt chart that sits on the Windows desktop. Its Microsoft Store publisher display name is 白描ice. The project is maintained at https://github.com/Lianzy-Baimiao/FluentCal .

### Your tasks and settings

FluentCal requires no account and includes no advertising, usage analytics or automatic crash-report upload. The application does not send your tasks, settings, usage history or device identifiers to the developer.

Tasks, groups and settings are stored on this PC in `data.json`, with a daily backup in the `backups` folder beside it. The standalone installer uses `%APPDATA%\com.fluentcal.desktop`; the Microsoft Store edition writes to its package's own data folder. In Settings › Data you can move the data folder elsewhere (for example a OneDrive folder); the service behind that folder then handles the data according to your settings. Exports (JSON, CSV, ICS, Gantt images) are written only to files you choose. Uninstalling the standalone edition keeps your data; uninstalling the Store edition normally removes the package's data.

### Features that use the network

- **Public holiday updates** (on by default; Settings › General): public holiday data (NateScarlet/holiday-cn) is downloaded from fastly.jsdelivr.net or raw.githubusercontent.com. The requests contain none of your tasks or settings, but those services can see your IP address and the time of the request.
- **Cloud sync** (off by default): runs only after you enter your own WebDAV storage or GitHub / Gitee repository in Settings › Sync. Your data travels directly between your PC and the service you chose, never through the developer. The cloud copy is plain JSON, protected by that service's access control. Passwords and tokens are kept in Windows Credential Manager, not in the data file.
- **Web view** (off by default): "View in browser" starts a read-only page that listens on 127.0.0.1 only. If you turn on "Allow devices on the local network", devices on the same network can view your calendar read-only with an access code; the code is kept in Credential Manager and can be reset or turned off at any time.
- **Android QR pairing** (started by you): the desktop temporarily serves the selected cloud configuration and credentials over the local network, encrypted with AES-GCM for the phone that scans the code. The code contains the decryption key and expires after five minutes or one retrieval. Canceling, refreshing or leaving the Sync page revokes it. Pairing data is not sent to the developer or written to task data or logs. Only let your own devices scan the code.
- **Android client**: tasks and settings are kept in private app storage. Cloud credentials are encrypted with Android Keystore and stored outside system backups. The phone connects directly to your chosen cloud service. Camera access is used only to scan codes; camera images are not saved or uploaded. You choose where to send exported files through the system share sheet. Uninstalling normally deletes local app data.
- **Update checks**: the desktop queries Microsoft Store; Android queries this project's public releases on GitHub. Requests contain no tasks or cloud credentials, but the service can see the request IP address and time. Downloading the mobile app or an update opens the GitHub release page.
- Links you choose to open (project page, help pages) are handled by your browser and those sites under their own policies.

### System features

Reminders use Windows notifications. Start with Windows uses a Windows startup entry (the app's startup task in the Store edition) and can be turned off in Settings. Microsoft handles Store accounts, distribution, updates and diagnostics under its own privacy statement; FluentCal does not control these services.

### Contact and changes

Contact the maintainer through https://github.com/Lianzy-Baimiao/FluentCal/issues . Issues are public: do not post passwords, tokens or task content you don't want to share. This policy and its effective date will be updated if data handling changes.
