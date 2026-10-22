# 真实 Codex 身份识别验收（2026-10-20）

结论：**部分通过，发现阻断问题。** 默认模式下真实 Codex 的会话无法关联到托管终端；加 `--no-daemon` 后关联、同终端先后两段会话、后台重启保留均通过。

## 环境

- Codex CLI 0.163.0（npm 安装，`codex` 是 node 包装器，实际进程为原生二进制）；用户 shell 别名为 `codex --dangerously-bypass-approvals-and-sandbox`。
- keeplined：`feat/agent-workbench` 工作树当时的 debug 构建，独立运行目录 `/tmp/kla`，日志根为真实 `~/.codex/sessions`、`~/.claude/projects`（只读）。
- 测试 Hook：临时加入 `~/.codex/hooks.json` 的 SessionStart / UserPromptSubmit / Stop 三条，经用户授权在 Codex 中信任；验收后已删除，`hooks.json` 与 `config.toml` 已逐字恢复为测试前内容。未改动 Claude 配置。

## 结果

| 项 | 结果 | 证据 |
|---|---|---|
| 首次索引 | 通过，但读量大 | 1590 个目录、5538 个日志、读 689 MB、写索引 7133 次，约 3 秒 |
| 默认模式 `codex "…"` 的会话关联 | **失败** | Hook 被拒：`Hook process is not a descendant of a managed terminal` |
| 失败原因 | 已确认 | Codex 0.163 默认把回合交给共享的 app-server 守护进程执行，Hook 与工具命令的进程链是 `sh → zsh → codex app-server（pid 3836）→ launchd 下的 daemon`，不在托管 PTY 里；PTY 里只有 TUI 客户端 |
| `codex --no-daemon "…"` 的会话关联 | 通过 | Hook 进程链 `sh → zsh → codex → node → 托管 zsh → keeplined`；会话 `01a1590d-…` 绑定到终端 t1 |
| 同一终端先后两段会话 | 通过 | 第二段 `01a1590e-…` 绑定 t1，第一段 `current=false`，两段各自 `Open` |
| 后台重启 | 通过 | 终端标为 Interrupted（原 PTY 不可重连），两段会话与归属保留 |
| 外部会话不冒充托管 | 通过 | 测试期间用户其他目录的 Codex 会话 Hook 被拒绝，未写入工作区 |
| Agent 退出后的“当前”标记 | **未通过** | 第二段会话退出（Ctrl-C 两次）后仍为 `current=true`，直到后台重启；Codex 没有 SessionEnd Hook，需要用进程退出来结束 |

## 决定（2026-10-20，用户）

首版**暂用 `--no-daemon`** 作为过渡方案，用户明确表示这不是好方案：Keepline 预填的命令（从待办开始、继续会话）一律带 `--no-daemon`；用户自己输入的 `codex` 识别不到时，在画面状态栏提示“改用 codex --no-daemon 才能跟踪”。同时调研不依赖进程树的关联方式，找到后去掉这个限制。状态：待验证。

## 需要处理

1. **默认（守护进程）模式的关联**：进程祖先链在真实 Codex 上不成立。可选方向，需要设计决定：
   - Keepline 预填的命令统一加 `--no-daemon`，并在用户自己输入 `codex` 时提示或在识别失败时给出“改用 --no-daemon”的提示；
   - 或找到不依赖进程树的可靠证据（例如 TUI 客户端与 app-server 线程的对应关系），不能退化为按目录猜测。
2. **Agent 进程退出**：托管终端中 Agent 进程消失时，应把对应会话的 `current` 置为 false。
3. **Hook 负载含用户原话**：UserPromptSubmit 的负载包含 prompt 原文，且全局 Hook 会收到所有 Codex 会话（包括非托管的）。后台必须在确认托管之前不记录、不落盘负载内容（当前实现是先拒绝，符合要求；测试用的调试抓包已删除）。
4. 首次索引读量 689 MB，虽然只发生一次，仍应确认“跳过大正文”是否按预期生效。

## 未覆盖

Claude Code（按用户要求不改 Claude 配置，未测）；中文输入法与 GPUI 原生交互（待用户一起手测）；长期功耗。


## 后续修正（同日，视觉重做轮）

已实现预填/续接命令构造带 `--no-daemon`、未关联托管 Codex 的状态栏提示，以及 kqueue NOTE_EXIT 清除对应 AgentSession.current。两项生命周期/提示集成测试通过，终端仍运行、task/outcome 与 SQLite 落盘均有断言。原生截图和其余边界见 [视觉验收记录](visual-20261020/README.md)。这是实现与模拟测试证据，**没有把本页原来的官方 CLI 退出未通过记录改成通过**；官方 Codex 路径还需再次手验。默认 daemon 模式可靠关联、首次索引 689 MB 的读量分析、Claude 配置和长期功耗仍未解决或未覆盖。
