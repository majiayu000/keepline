# 会话存活与工作状态：同类实现调研

记录时间：2026-10-09 17:48 +08:00。

## 问题与当前实现

用户希望最新活动优先，并希望知道哪些 Codex 会话已经关闭；不能把历史未查看回复数量当成当前打开的对话数量。

Keepline 的 `src/services/session.service.ts` 已经检查关联 PID 是否还在扫描集合中，不在则写为 `lost`。但 `src/services/ledger/service.ts:refreshStatus` 又按回合完成、未查看和验收情况推导总览状态：已完成但未查看的回合继续显示 `review`。因此，“进程是否存活”和“本轮工作是否完成”在总览里混在了一起。

当前规则：

- `review`：本轮完成且未在 Keepline 查看，或完成未满 12 小时且有验收项。
- `ended`：本轮完成，且不再满足上述 review 条件。
- `stopped`：回合被中断，或本轮未完成且底层状态为 lost / stalled / interrupted；目标受阻或额度用尽也可能进入此状态。
- 未查看回复不会因超过 12 小时自动消失。Keepline 不能知道用户是否已经在原应用查看回复。

已经完成的本轮修改：默认提供最新活动排序；总览明确标为“会话记录”；进程解析保留可执行文件名大小写，修复带空格的 `Codex Computer Use`、`Codex Framework`、`Claude Helper` 路径被截断后误识别为 CLI 的问题。

时间点观测：修复前扫描出 102 个 CLI 候选进程，修复后为 28 个（23 Codex、5 Claude）。这些是进程数，不是对话数，不能当成长期固定数据。服务重启后最近 24 小时有 76 条会话记录，其中 49 条 review、22 条 stopped、5 条 running。此时 API 已验证最后活动时间倒序。

## 同类软件

### Herdr

- 本地参考仓库 `/Users/lifcc/Desktop/code/AI/tools/herdr`，HEAD `9eb5214`，v0.8.2；相关源码只读查看，未修改。
- 管理 PTY 终端；macOS 从终端 shell 的前台进程组获取 Agent 进程。
- Codex 退出、前台回到 shell 时，先发布进程退出的 idle 转换，再清除当前 Agent 身份。整个窗格子进程退出由 `child.wait()` 观察。
- Codex 工作状态主要来自终端画面和 OSC 标题；SessionStart hook 通过本地 socket 关联原生会话 ID。
- 源码：`src/platform/macos.rs:foreground_job`、`src/pane.rs:foreground_shell_agent_action`、`src/pane/agent_detection.rs:detection_update_for_publish_with_osc`、`src/integration/assets/codex/herdr-agent-state.sh`。
- 官方说明：<https://herdr.dev/docs/agents/>、<https://herdr.dev/docs/integrations/#codex>。

### ccmux

- 调研远端 HEAD：`0a0c4fa93927`。
- hook marker 关联原生 session ID、PID、TTY；再关联 tmux pane。
- 活跃 PID 消失则清理会话；PID 存活但 pane 消失只解除 pane 绑定。
- 清理有连续两次扫描确认；扫描整体失败应跳过清理，不能把探测失败当进程退出。
- 参考：<https://github.com/epilande/ccmux/blob/0a0c4fa93927/src/daemon/binder/cleanup.ts>。
- 架构说明：<https://github.com/epilande/ccmux/blob/main/docs/architecture.md>。

### AgentMonitor（Ericonaldo）

- 调研远端 HEAD：`38e2085f112b`。注意与 `jiweiyeah/AgentMonitor` 是不同项目。
- 外部 Agent 通过进程扫描发现，使用扫描结果或 `process.kill(pid, 0)` 检查存活。
- 外部进程确认退出后删除仪表盘上的 external Agent 行；内部创建的 Agent 停止后保留。
- 源码：<https://github.com/Ericonaldo/AgentMonitor/blob/38e2085f112b/server/src/services/ExternalAgentScanner.ts#L118>。
- “删除行”不代表删除原始 Codex / Claude 对话文件。

### cmux

- 调研远端 HEAD：`9b3e17813ece`。
- hook 关联 session ID、PID、workspace 和 surface，记录 running / idle / needsInput / unknown。
- 休眠明确区别于空闲：可以终止空闲后台进程，之后恢复原会话。因此“会话记录存在”不意味着“进程仍然存在”。
- 文档：<https://github.com/manaflow-ai/cmux/blob/main/docs/agent-hooks.md#agent-hibernation>。

### Agent Deck

- 调研远端 HEAD：`34cf3690c396`。
- 使用 tmux 托管；Codex 回合状态依赖 notify hook。归档停止 tmux 进程并隐藏默认列表，保留对话和元数据。
- 文档：<https://github.com/asheshgoplani/agent-deck#archive-sessions>。

## 实现方向（2026-10-09 已落实）

将进程存活与工作回合分开显示：

| 进程证据 | 回合状态 | 显示建议 |
| --- | --- | --- |
| 明确存活 | 执行中 | 执行中 |
| 明确存活 | 已回复 | 空闲 / 等你处理 |
| 明确退出 | 已回复 | 已退出 · 有未查看回复（如果存在） |
| 明确退出 | 未完成 | 已退出 · 本轮未完成 |
| 无准确绑定或探测失败 | 任意 | 在线状态未知 |

- 活跃列表以准确的存活绑定为依据；历史会话和未查看回复另行保留，不算成打开的对话。
- 独立 CLI 的 PID 退出可确认该次运行结束；需要防止 PID 复用或误绑定。
- 桌面 Codex 共享后台进程：持有 rollout 文件是加载证据，缺少句柄不是关闭的充分证据；不能把共享 PID 直接当成每个对话的独立进程。
- 不因为长时间无日志就声称对话已关闭；不把 Stop / 回合完成事件当成进程退出。

## 验证与试用边界

竞品对比采用文档与源码核对，没有安装或运行竞品。Keepline 实现通过 162 项相关回归（26 个文件）、4 项浏览器测试（含 1440px / 390px）、前后端类型检查、浏览器测试类型检查和前端生产构建。

用户已明确要求把分析实现在 Keepline，未要求安装其他工具。

实际实现：`sessions.process_state` 独立记录 `online / exited / unknown`；成功绑定 CLI 或 rollout 句柄时记在线，只有之前绑定的 PID 经系统探测返回 ESRCH 才记退出。句柄消失但共享 PID 仍在、权限错误、扫描失败均为未知。重启撤销在线结论，同时保留 PID 供新扫描核对；不会把重启当作 Agent 退出。旧记录默认未知，不回填猜测。

界面分别统计在线、已退出、待确认，最新活动排序保留。已退出记录收进历史，未读回复和验收操作保留；退出时未结束的回合标为停下，不冒充正常完成。过期输入请求不再触发通知。

退出实验使用隔离配置和独立 sleep 子进程：重启对账后 PID 存活但未匹配 → 未知并保留 PID；终止测试进程 → 已退出并清 PID；下一轮扫描仍为已退出。未停止用户的 Agent，也未修改用户 hook。

2026-10-09 18:20 本地服务已重启，实际生产页面和认证 API 均验证：最近 24 小时 73 条记录，29 条在线、44 条未知、0 条明确退出；按最新活动排序。该快照仅代表当时进程关联，不是打开的对话窗口数。旧记录没有可靠 PID 时保持未知；不因缺少在线证据就推断退出。
