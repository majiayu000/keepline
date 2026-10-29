# Agent 与终端的绑定

## 要支撑的决定

接一种新的 CLI Agent 时，只写一个适配器加一份真实日志样本，不改绑定逻辑，也不改界面。

放弃条件：如果接第三种 CLI 时，除了加一种新线索外还得改绑定器或界面，说明分层没有切对，就停下来重新设计。

## 起因

2026-10-29 实测：Keepline 终端里手动敲的 `claude`（不带 `--session-id`）一个都没进左侧。数据库里 711 个 Claude 会话从来没有绑到过终端；Codex 有 20 个绑上了。

原因有三个，叠在一起：

- 屏幕配对写死只给 Codex 用；
- Claude 不会一直开着对话文件；
- 没有安装任何 hook。

## 两层

1. **适配器**（`keeplined/src/agents/*.rs`，已在 476386b 落地）：只描述一种 CLI 的事实，包括日志在哪、怎么认出它的进程、记录怎么解析、命令行里的会话 ID、恢复命令，以及 hook 配置文件放在哪（`Adapter::hooks`，不支持 hook 的 CLI 返回 `None`）。
2. **绑定器**（`recognition.rs` + `host.rs`）：不区分 CLI。对每个终端里的 Agent 进程，从最强的线索开始逐条尝试，第一条对上就绑定；出现歧义时宁可不绑。

| 线索 `Binding` | 来源 | 什么时候能用 |
|---|---|---|
| `Process` | 命令行 `--session-id` / `resume <id>`（Keepline 新建 Claude 时会预先指定 ID），或 CLI 自己的运行中会话登记表（Grok） | 进程一启动 |
| `Hook` | CLI 的 hook 执行 `"$KEEPLINE_HOOK" hook --agent <cli>`，上报会话 ID 和 `KEEPLINE_TERMINAL` | 启动时（SessionStart），早于对话文件写出 |
| `OpenFile` | 进程一直开着对话文件（lsof） | 进程打开文件之后 |
| `Screen` | 最新一句提示词只出现在一个同类 CLI 的屏幕上，或者回车时间对得上 | 发出第一句话之后 |

## 契约

- 每个终端的环境里都带 `KEEPLINE_TERMINAL`、`KEEPLINE_RUNTIME`、`KEEPLINE_HOOK`（`keepline_core::protocol`）。hook 命令只安装一次、所有版本通用，不在 Keepline 终端里时什么也不做。
- hook 命令读完 payload 后转到后台上报，立即返回，不拖慢 CLI（实测一次上报约 2 秒，挂在 UserPromptSubmit 上会让每次发消息都多等这么久）。所以进程树通常已经断开：以环境变量里的终端 ID 为准；进程树还在时两者必须一致，不一致就拒绝绑定。后台盯着这个终端里唯一一个同类 CLI 进程的退出。
- hook 可能早于对话文件到达：这时用 hook 自带的 ID、目录和路径先绑上，等文件写出后由索引补上内容，会话 ID 不变。
- 屏幕配对只在同一种 CLI 之间进行：`ScreenTerminal.runtime == Transcript.runtime`。
- 只靠屏幕配对（或者没有记录线索来源）绑上的会话，允许在同一个 TUI 里切到新的对话（`/new`、`/clear`）。由进程报出的 ID、hook 或打开的文件绑上的会话不参与重新配对。
- 线索来源存在单独的 `agent_bindings` 表里，不给 `agent_sessions` 加列，这样交接期间的旧后台写入不受影响。
- 认出了 Agent 但还没对上对话的终端，会带上 `AGENT_UNBOUND_PREFIX` 提示。左侧把它列成一行，窗格上也会说明原因。认不出来的会话不能消失不见。

## 新增一种 CLI

1. 在 `Agent` 枚举里加一个变体，写上启动和恢复命令；
2. 新增 `agents/<cli>.rs`，实现 `Adapter`。如果这个 CLI 读命令行 hook，就实现 `hooks()`；
3. 补一份脱敏的真实日志样本；
4. 不需要改 `recognition.rs`、`host.rs` 和界面。如果不得不改，就触发上面的放弃条件。

## Hook 安装

`keeplined hooks install|uninstall|status [--agent <cli>]`。改写 CLI 自己的配置文件时保留原有键的顺序，只增删带 `$KEEPLINE_HOOK` 的条目，写入时先写临时文件再改名替换。配置文件不是合法 JSON 对象时不做任何改动。

## 决定

- 采用：两层结构、四条线索、终端环境变量和通用 hook 命令。
- 待验证：在真实 Claude Code 上装好 hook 后的启动绑定（需要改用户的 `~/.claude/settings.json`，等用户同意）。Grok 和 Codex 的 hook 暂不接：它们靠 `active_sessions.json` 和打开的文件已经能对上。
