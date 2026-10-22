# A04：Claude 审批只出现在画面上时的原生复验（2026-10-22，无模型）

**结论：隔离 runtime 里的原生链路通过。** 用的是 R9 真实 Claude Code 2.1.302 审批画面的原文，会话通过真实的 `--session-id` 关联，JSONL 停在一个没有权限记录的 Write 调用上。后台在画面出现后约 1.0–1.3 秒把它改判为“等你回答”，原生界面的各处入口一致。**仍未验证**：用真实 Claude（消耗一次模型回合）在用户自己的工作台里触发审批。

## 夹具

[fixture.py](fixture.py)：
- 画面 p1 运行 `claude -c <script> claude --session-id <uuid>`，其中 `claude` 是指向 /bin/sh 的替身。脚本先写入 JSONL（一条 `entrypoint=cli`、`userType=external` 的用户消息，加一个待执行的 `Write` tool_use），等关联完成后，原样输出 R9 截图转写的审批画面。
- HOME 放在 /tmp（客户端单实例 socket 路径受 SUN_LEN 限制），项目和 runtime 放在 `~/Library/Caches/KeeplineAcceptance`。原因是 Claude 会话的 cwd 在 /tmp 时会被判为“临时目录会话”并隐藏，见下方体验问题。
- 不读凭据，不调用模型，结束时按 ID 停止终端和后台，并删除夹具目录。

## 证据

- [receipt.json](receipt.json)：关联时 `turn=Running, waiting=""`；画面出现 1.13 秒后（最后一轮）为 `turn=Waiting`、`waiting="Do you want to create approval-proof.txt?（画面上的提示）"`。各轮分别为 1.03、1.04、1.27、1.13 秒。
- [native/screenwait-native.png](native/screenwait-native.png)：GPUI `render_to_image`，1440×900 @2x。可见：标签角标 1、空间角标 1、左栏“需要你 · 1”、画面状态栏“等你回答 · 等待输入…（画面上的提示）”、“你在这里”卡的“停在 / 下一步 · 推断：回应上面的审批或问题”，以及审批原文。
- [native/overview-native.png](native/overview-native.png)：鸟瞰视图。

## 过程中发现

- 第一次把项目放在 /tmp：会话被隐藏为“临时目录会话”，“需要你”为 0，审批不提示。这是产品行为，不是夹具故障：在 Keepline 自己的画面里、用 `--session-id` 明确启动的会话，只因工作目录在 /tmp 就被隐藏。已记入体验问题清单，未改。
- 截图中的“你在这里”卡在会话没有任何结局时也显示“撤销结局”按钮，已记入体验问题清单。
