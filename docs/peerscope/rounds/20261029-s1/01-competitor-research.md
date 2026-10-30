# Warp 设置模块对照

观察日期：2026-10-29。以下为官方文档与本机包内配置契约证据，未操作 Warp GUI。

| Warp 的设计 | 证据 | Keepline 本轮选择 |
|---|---|---|
| 通用、外观、终端、Agent、快捷键、通知分组 | [完整设置参考](https://docs.warp.dev/terminal/settings/all-settings/) | 按当前能力分成通用、外观与终端、快捷键、通知 |
| 字体、字号、行高可以单独调整 | [文字、字体与光标](https://docs.warp.dev/terminal/appearance/text-fonts-cursor/) | 等宽字体选择、字号增减、四档行距、使用实际终端画笔的预览 |
| 图形设置与配置文件表示同一套偏好，修改即时应用 | [设置文件](https://docs.warp.dev/terminal/settings/) | 控件保存后立即重绘，终端缩放快捷键写入同一个字号设置 |
| 为启动行为与通知提供偏好 | [完整设置参考](https://docs.warp.dev/terminal/settings/all-settings/) | 接入已有 default_agent、away_minutes、stale_days、notifications |

本机 Warp 安装包版本为 `0.2026.10.07.08.29.00`。
检查安装包附带的设置 schema，确认其中含 general、appearance、keys、notifications 分组；
appearance.text 有字体和行高字段，通知配置有事件偏好。
包内契约不证明每个选项在本机 GUI 都已成功工作。

## Keepline 现状核对

本轮开始时，原生 SettingsView 有快捷键页与明暗页；字体选择、字号和行距设置未接入。
core 的 Settings 已有默认 Agent、通知、收起天数、离开分钟数；没有必要再加一套数据模型。
桌面通知的实际触发逻辑是等待输入与错误，因此说明不承诺完成通知。

## 关键事实核验

| 事实 | 影响 | 来源及核验结果 |
|---|---|---|
| Warp 有通用、外观、通知、快捷键配置 | 分类设计 | 官方完整设置参考及本机 schema，成立 |
| Warp 支持字体与行高设置 | 文字设置优先级 | 官方字体页面及本机 schema，成立 |
| Keepline 已有四项通用偏好 | 避免重复建模 | core/model.rs 的 Settings 与 Edit::Settings，成立 |
| Keepline 通知是否含完成 | 界面说明 | desktop.rs 的 fact 仅有 Waiting、Error，更正为仅这两类 |
| Keepline 当前本地代码是否只有快捷键 | 基线 | SettingsView 已有明暗页，更正为快捷键与基础明暗 |

不将 Warp 的 Agent 权限和模型设置映射成 Keepline 假开关。它们涉及不同产品能力。
