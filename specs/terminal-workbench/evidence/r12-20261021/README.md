# R12：原生通知审计与屏幕共享抑制

**A09 仍未完整通过。** 请求被系统接受、进入 delivered list，不等于用户能看到。用户明确反馈 14:08 的通知在横幅和通知中心均找不到；进一步查到 donotdisturbd 的明确抑制原因。

## 隔离范围

测试 bundle `local.keepline.notification-test`，Developer ID 签名；客户端 PID 93389，runtime `/Users/lifcc/Library/Caches/KeeplineAcceptance/r5-bio55vqi/runtime`。仅用私有 JSONL 合成等待/错误事件，不调用模型、不读用户对话、不改用户后台或系统通知权限。

新增本地 os_log 审计：request_completed、delivered_list、will_present、did_receive、route_applied。只记 bundle/请求/会话/终端/标签 ID、动作与错误码，不记录通知正文或对话。route_applied 仅在选中对应标签、存在 pane 且 focus_handle 已聚焦时记录，不能用排队中的路由意图替代。

## 分层结果

| 项 | 结果 | 证据 |
|---|---|---|
| 等待/错误请求接受 | 通过 | 14:08:36 / 14:14:04 accepted=1 |
| OS delivered list 与 usernoted 投递 | 通过（仅系统层） | system-events.json 中 delivered_list、Delivering、Presenting |
| 用户实际可见 | 未通过原始尝试；重发待确认 | 用户报告没找到；不能把 Presenting 当可见 |
| 重复事实去重 | 通过 | 等待/错误各追加同一事件三次，每类仍仅一个 request_completed；receipts.json |
| 真实 didReceive + 直达画面 | 未验证，用户无需操作 | 未模拟回调；before-click.png 仅点击前状态 |

## 找到的原因与设置

- 系统设置中测试应用允许通知=on、桌面/通知中心/锁屏=1，提醒样式起初为临时，检查期间外部操作改为持续；工具未更改开关。见 notification-settings.txt。
- 全局「当镜像或共享屏幕时」显示「关闭通知」。
- donotdisturbd 对两条通知均明确记录 `outcome: suppressed; reason: display shared; activeModeUUID: (null)`。所以是屏幕共享策略抑制，不是普通命名专注模式，也不是未授权；见 suppression-events.json。14:14:51 后同一事件重新计算为 allowed/disabled。
- 14:17:05 已重发唯一编号 1792563425，尚无用户可见/点击确认，不再要求用户操作。若仍被抑制，请用户在「系统设置 → 通知 → 当镜像或共享屏幕时」自行选择允许通知；无需重复改应用允许通知。
- 读取 DoNotDisturb Assertions.json 被系统拒绝，未绕过权限；随后使用用户要求的系统日志与原生设置只读检查。

查询：`log show --start ... --style json --predicate 'subsystem == "local.keepline.desktop" OR ((subsystem == "com.apple.UserNotifications" OR process == "usernoted") AND eventMessage CONTAINS[c] "local.keepline.notification-test")'`；补查 donotdisturbd 的 suppressed/display shared/activeModeUUID。仅保存目标 bundle 的精简事件，不提交其他应用日志。

## 检查

release 客户端构建与正式签名 verify 通过；fmt、workspace clippy -D warnings、客户端 24 项测试通过。系统可见性/点击是独立验收，未由单测代替。点击验收保留未验证，不要求用户操作，不操作用户 Keepline。
