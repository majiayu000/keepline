# M4 菜单栏、通知与设置

决定：采用 AppKit 原生状态项/弹出面板；设置和通知去重落库，无周期轮询。通知默认关闭。

| 项目 | 结果与证据 |
|---|---|
| 原生面板等待/出错计数、问题与选项摘要 | 通过，Computer Use；[面板](menubar.png)。通过应用菜单“菜单栏面板”打开同一个 NSPopover，未单独验证状态栏图标点击 |
| 点击错误会话，无主窗口时重开并直达 | 通过，Computer Use；[跳转](menubar-route.png)；原有隔离 PTY 保留 |
| 设置天数、离开阈值、默认 Agent | 通过，Computer Use；[设置](settings.png)、[SQLite 读回](settings-readback.json)；新待办默认 Claude |
| 默认关闭、相同事实去重、新问题重新通知、完成不通知 | 通过，单元测试；不等于真实系统通知验证 |
| 设置与去重重启持久化 | 通过，SQLite 单测 |
| 系统通知授权、实际展示及点击路由 | **未验证，等待用户授权**。用户要求不改系统权限；未开启通知授权 |

测试：core 17、daemon 24、client 15 通过，原生桥接构建通过。日志见 tests-core-daemon.txt、tests-client.txt。模拟日志与隔离后台，不调用模型、不动用户后台。发现错误卡展示原始 JSON，已改成 error/message 摘要并补测试，最终截图为修复后。
