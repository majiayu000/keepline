# 不加 --no-daemon 时的 Codex 关联：开源方案调研

日期：2026-10-21。本机 codex-cli 0.163.0。

要支撑的决定：手动输入的默认（共享守护进程）模式 Codex，能否不靠 `--no-daemon` 就可靠关联到托管终端。
放弃条件：只能按 cwd、时间先后、标题前缀去猜；必须改用户的 Codex 配置；必须加代理。

## 共识

0.157 起 hook、MCP、工具命令都在共享 app-server 里跑，继承的是**最先启动它的那个终端**的环境变量。所有靠 `TMUX_PANE` / `HERDR_PANE_ID` 这类环境变量分辨终端的工具都坏了：Herdr、tmux 系、Orca、OpenRig、cc-deck、Maestri 等。OpenAI 维护者在 openai/codex#48500 回应：hook 跑在 app-server 里，和哪个客户端无关，这不是设计承诺。上游正式方案 openai/codex#44902（hook 范围的客户端环境白名单）仍为 OPEN。

Herdr 本身的会话关联（agent_session）同样失效（herdrdev/herdr#4649 OPEN、#4859 重复）。官方给用户的临时办法也是 `codex --no-daemon`。Herdr 状态显示正常，是因为状态只看画面和 OSC 标题，见 `~/.local/state/herdr/agent-detection/remote/codex.toml`。

## 各家做法

| 方案 | 代表 | 是否精确 | 对 Keepline 的适用性 |
|---|---|---|---|
| 退回 `--no-daemon` | Orca、OpenRig、svall、Herdr 官方建议 | 精确 | 已采用 |
| 由工具自己建 thread，再 `codex resume <thread> --remote` 接入 | Wirasm/helm#624（已合并，0.160 实测） | 精确，生成时就知道 thread | 只覆盖 Keepline 预填的命令；R3 对 `--remote` 的回调和恢复尚未证明 |
| hook payload 首个 `session_id` + 自建 thread→终端记录 | eneskirca/nodeterm#1232 | 精确，但前提是 thread 已登记 | 需要装 hook（改用户 Codex 配置），而且仍需先有登记 |
| `codex resume <id>` 进程参数 | muxa、Herdr 用户 wrapper | 精确 | **Keepline 目前只解析 Claude 的参数**（`recognition.rs:224`），Codex 未用 |
| 终端屏幕上的最近一条提示 ↔ rollout 最近一条用户原话，唯一时才绑定 | Open330/muxa#197（已合并，实测同 cwd、同毫秒双开都绑对） | 有条件精确；完全相同的提示、短于 12 字时拒绝绑定 | Keepline 自己就是终端，屏幕缓冲现成，不用改配置 |
| OSC 标题 `<thread_name> \| <folder>` ↔ `~/.codex/session_index.jsonl` | herdr#4649 评论（用户 wrapper） | 唯一时可用；首轮自动起名后才出现 | 本机 1314 条里有 21 个重名，必须加唯一性和 cwd 约束 |
| 只看画面判断状态，不关联 | Herdr、agent-deck 不装 hook 时 | 不关联会话 | 可作为兜底：显示状态但没有内容 |

## 决定

**采用（模拟验收通过；真实 CLI 待验证）。** 用户于 2026-10-21 同意第 2 层修改旧决定，三层都做。

1. `codex [选项] resume <UUID>` 参数精确绑定，node 包装进程和原生子进程只计一次。
2. 屏幕原话一一对应：去掉空白后至少 12 字，取前 160 字；比对本终端末尾 200 行；进程 cwd 与日志 cwd 规范化后须相同；`/new` 只允许较新的线程接管，且新原话要和当前线程的不同；每条原话最多比对 5 次。
3. 都不唯一时只按标题显示状态；带 `--no-daemon` 的进程不再提示。

## 验收（分支 feat/codex-daemon-association）

| 场景 | 集成测试 | 结果 |
|---|---|---|
| 同目录开两个 | `daemon_codex_same_cwd_panes_bind_to_their_own_screen_and_exit_clears_current` | 通过；退出后清除 current |
| 同一毫秒开两个 | `daemon_codex_threads_started_in_the_same_millisecond_bind_correctly` | 通过 |
| `/new` | `daemon_codex_new_hands_the_pane_to_the_newer_thread_only` | 通过；旧线程降为非 current，同目录空闲 Codex 不被分配 |
| 相同提示 | `daemon_codex_identical_prompts_stay_unbound_until_a_distinct_follow_up` | 通过：相同提示时两边都不绑定，各自追问后分别绑对 |
| 标题状态 / --no-daemon 不提示 | `unbound_daemon_codex_shows_title_state_and_no_daemon_shows_nothing` | 通过 |

反向检查：去掉一一对应限制后，相同提示测试失败，并出现 p0 被绑到 p1 线程的误绑定，说明测试能抓住误绑定。另有纯函数单测覆盖短提示、单线程显示在两屏、一屏显示两线程、较旧线程接管、当前线程原话相同等情况。keeplined 和 keepline-core 测试全部通过；工作区 clippy `-D warnings` 与 fmt 均通过。

测试使用伪造的 `codex`（指向 /bin/sh）：它在真实 PTY 上显示提示和标题，同时写入并立即关闭会话日志，模拟守护进程在终端外写日志。**这不等于真实 Codex 0.163 已验证**：真实 TUI 渲染用户原话的格式、粘贴大段文本时显示的占位符、首轮提示写入日志和屏幕的先后顺序都还没有实测。

## 已知限制

- 粘贴的长文本在 TUI 里显示成占位符，无法匹配，只能走第 3 层。
- 提示短于 12 字时不绑定。
- Keepline 之外的同目录 Codex，如果和托管终端在相近时间输入完全相同的长提示，且它的日志先于托管线程写入，仍可能误绑定。目录一致加原话完全一致是这里的最后一道防线，没有进一步的证据可用。
- 上游 openai/codex#44902 落地后，应改用官方提供的客户端身份，删除第 2 层。
