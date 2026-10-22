# R6 系统通知实测（2026-10-21）

决定：只用自己的隔离 runtime 与测试 bundle。请求系统授权后由用户处理；不能以应用内“通知开启”、模拟去重或菜单栏跳转代替系统投递与点击证据。不修改用户安装包、后台、终端或系统开关。

## 无弹窗排查

1. 最初 `Keepline Native Acceptance.app` 的 CFBundleExecutable 是 shell wrapper `launch`，ad-hoc 签名，位于 `~/Library/Caches/KeeplineAcceptance/`。应用设置切换调用原生 `kl_notifications_enable`，`UNUserNotificationCenter.requestAuthorizationWithOptions` completion 返回 `UNErrorDomain error 1`。SDK UNError.h 对应 `UNErrorCodeNotificationsNotAllowed`。[回执截图](wrapper-request-error.png)。这是请求实际执行的证据，不是仅保存设置。
2. 改用直接原生可执行文件的 `Keepline Notification Test.app`，CFBundleExecutable=keepline-app，独立 bundle ID `local.keepline.notification-test`。同 HOME/runtime、仍在 Applications 外。ad-hoc 包请求最终返回 `Notifications are not allowed for this application`，[截图](direct-request-state.png)。用户确认没看到弹窗，因此不能把窗口失焦当作许可框已经出现。
3. Computer Use 明确拒绝访问 `com.apple.UserNotificationCenter`。没有换用其他自动化绕过；仅查看自己的应用与用户授权排查的系统设置。
4. 系统设置真实 UI 已登记 **Keepline Notification Test**，**允许通知为 off**，[AX](system-settings-off.txt)、[截图](system-settings-off.png)。这解释当前无法投递，但不能反推首次为何没有可见提示，或断言用户曾点过拒绝。
5. 用指定 Developer ID Ma JiaYu (C5UWZ934C2) 重签同一测试包的客户端、后台、bundle，启用 hardened runtime/时间戳，深度严格校验通过，[签名记录](signed-test-package.json)。重新启动原生窗口并发出请求，[签名包请求](signed-request.png)。没有换 bundle ID 来规避既有权限决定，也没有移动用户 App。

[Apple 官方说明](https://developer.apple.com/documentation/usernotifications/asking-permission-to-use-notifications)：系统记录授权答复，后续 requestAuthorization 不会再次提示，用户可在系统设置更改。现有证据不足以认定 ad-hoc 签名或 Applications 外路径是根因。

已请用户操作具体路径：**系统设置 → 通知 → Keepline Notification Test → 允许通知**。用户回复“已打开允许通知”；操作由用户完成。其后所有原生窗口出现 cgWindowNotFound，经只读系统状态确认 CGSSessionScreenIsLocked=Yes。未解锁、不修改锁屏设置。

## 投递与点击

待验证：等待提醒、错误提醒、点击直达对应托管画面、相同事实去重。`fixture.py` 已准备真实托管 PTY（模拟 Codex 名称、持续打开日志、无需模型），初始状态 Running；用户确认允许后已追加等待事件，后台 Waiting、正确托管关系和去重记录见 [waiting.json](waiting.json)。这只证明识别/去重，不证明系统展示或点击。锁屏阻塞解除后继续原生链路。

当前结论：A09 **未验证**（等待解锁后完成系统展示、点击与去重原生检查）。已验证请求确实执行、拒绝回执、设置登记、Developer ID 签名包启动、用户明确开启授权与后台 Waiting/托管身份。保留测试客户端供续验，不清空通知或修改用户系统开关。

## 本轮解锁补验

用户确认屏幕解锁且许可开启后，向原有隔离 fixture 追加错误事件；后台 Error 与通知 receipt 的 fact_key 正确更新，见 [读回](unlocked-error-readback.json)。测试应用设置仍显示“系统通知已开启”。没有更改系统权限、用户配置或触碰用户后台。

Computer Use 可连接 `com.apple.notificationcenterui`，但只返回“月”日历小组件，未取得通知列表/横幅；`com.apple.controlcenter` 与 `com.apple.systemuiserver` 均 timeoutReached。没有用其他 UI 技术绕过，也没有把数据库 receipt 当成系统已展示。A09 **仍未验证**展示、点击直达和系统级去重；阻塞已从锁屏变为通知界面不可访问。用户无需重复打开权限。
