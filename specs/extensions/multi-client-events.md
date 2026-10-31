# 多客户端与事件订阅：开工说明

状态：已排期，分三步做（2026-10-31）。用户确认大功能从这一项开始。SSH 多机、Windows 排在「首个可验收本地 App」里程碑之后，**这一项只在本机 Unix socket 上做，不加网络层**。

## 要支撑的决定

手机端、网页端、脚本和别的 Agent，能不能不改 App、只连 `keeplined`，就**知道每个会话的状态变化**，并**看到、操作某个终端**。这是拓展第 7 项（手机处理等你的会话）和以后 SSH 多机的共同底座；Herdr 的第三方手机端 Ovis 就是靠它的事件订阅和终端桥接做出来的（`~/peerscope/keepline/rounds/20261030-s2/evidence/herdr/card.md` H2）。

## 现在已有

| 能力 | 在哪里 | 缺什么 |
|---|---|---|
| 同一终端多个连接：一个可写，其余只读，可接管 | `host/live.rs` `attach` / `reconcile_owner` | 只读连接也会按自己的窗口大小调终端尺寸；没有「最后操作的那个调尺寸」 |
| 给 App 的推送：识别状态和 `workspace_revision` | `Request::WatchRecognition` | 只推「有变化」，客户端要再取整个 Workspace；格式是内部的 |
| 会话进入 开始 / 等你 / 出错 / 做完、命令结束 | `plugin_hooks.rs` `Hooks::changes`、`command` | 只交给插件命令和 webhook，不能订阅 |
| 读画面、发文字、等状态、答审批 | `control.rs`（list / read / send / prompt / wait）、`AnswerApproval` | 都是一问一答，没有持续的画面流 |
| 不改尺寸的只看连接 | 浮层 Peek（`Pane.peek`） | 只在 App 里，协议上没有这个模式 |

## 分三步

### 第一步：事件订阅（`keeplined events`）

- 新请求 `Subscribe`。连接后先回一份快照：终端列表，以及每个会话的状态和一行原因。之后同一个连接持续推事件，直到客户端断开。
- 事件：
  - `session.state`：working / waiting / idle / done / error，附原因一行；等你时附审批原文；
  - `approval.shown` / `approval.gone`：带 `question`；
  - `terminal.started` / `terminal.exited`；
  - `command.done` / `command.failed`：复用插件钩子的负载，不给命令行；
  - `lagged`：客户端跟不上时发一条，客户端收到后重新取快照。
- 来源和插件钩子同一处（`Hooks::changes` 与 `command`），不另写一套状态判断。没有订阅者时零开销。
- 每个订阅者一个有界队列，满了就标记 lagged，丢掉积压，不会卡住后台。断开靠阻塞读感知，不用心跳和定时器（`watch_recognition` 已经是这个做法）。
- 命令行：`keeplined events [--json]` 每行输出一个事件，给脚本和 Agent 用；`--skill` 补用法。
- 内容边界与插件钩子一致：只给卡片字段（标题、目录、空间、一行说明、审批原文），不给对话原话全文。

### 第二步：终端看 / 控桥接

- `Attach` 增加只看模式：不成为可写者，也不调尺寸（把 App 浮层的做法放到协议层）。
- `keeplined watch <终端>`：按画面变化输出纯文字快照，合并成最多每秒若干帧，第三方客户端不用自己实现终端模拟器。
- 操作仍走现有的 `send` / `prompt` / 答审批，不新开写入通道。

### 第三步：多客户端调尺寸

- 同一个终端被几个可写客户端同时看时，按最后操作（敲键、点击）的那个客户端调尺寸，对齐 Herdr 0.9.0。只看连接永远不调尺寸。
- App 只有一个实例，所以这一步在本机主要服务「App 加上手机或网页」同时开着的情况。

## 验收

| 指标 | 门槛 | 怎么量 |
|---|---|---|
| 不漏事件 | 端到端测试里制造 开始 → 等你 → 答审批 → 做完，以及命令成功 / 失败；订阅者按顺序全部收到，0 漏、0 重复 | `managed_session` 新用例 |
| 延迟 | 状态提交到订阅者收到 ≤ 200 ms（p95） | 同上，记时间戳 |
| 慢订阅者 | 一个永远不读的订阅者在场时，App 的画面和其他订阅者不受影响；它收到 `lagged` | 构造用例 |
| 没人订阅时 | 空闲 CPU、GPU 基准与 main 持平（`eval/gpu-bench` idle / hidden4 / vis1） | 基准对照 |
| 10 个订阅者 | 后台 CPU 增量 < 1% | 浸泡 10 分钟 |
| 只看连接 | 只看客户端连上、断开，终端尺寸都不变；可写者不受影响 | 端到端，对照 `stty size` |

## 放弃条件

- 第一步做完两周内，没有任何实际使用者（手机推送原型、脚本或 Agent 都算），就停在第一步，第二、三步不做。
- 订阅让后台空闲 CPU 或 GPU 基准变差且压不回去，就退回只给插件钩子。

## 不做

网络传输（TCP、SSH、TLS）、账号与配对、跨机器同步，都留给里程碑之后的多机方案。本机只有当前用户能连 socket，沿用现有的 peer uid 检查。

## 收尾时写

每一步结束在本文件末尾写：采用 / 否定 / 待验证，附测试与基准数字。

## 第一步结果（2026-10-31）：采用

- `Request::Subscribe` / `Reply::Events`，命令行 `keeplined events [--json]`，`--skill` 已写用法。状态事件直接复用插件钩子的判断（`Hooks::changes`），订阅者用自己的一份记录，和插件互不影响；审批框出现 / 消失看画面规则的 `approval`；终端开始 / 结束看终端列表；命令事件和插件钩子同一个出口，同一次读输出里结束的命令合成一帧。
- 有订阅者时，终端会读 shell 的命令标记，即使没有插件听命令事件（全局开关 `SUBSCRIBED`，读输出的线程不碰后台锁）。
- 端到端 `events_stream_every_change_once_in_order_and_a_stalled_reader_blocks_nobody`：终端开始 → 等你 → 审批 → 审批消失 → 做完 → 命令成功（时长 ≥ 150 ms）→ 命令失败（退出码 3）→ 终端结束，按顺序收到；真的 `keeplined events --json` 进程也收到这些。答审批到收到「审批消失」1.5–1.9 ms（门槛 200 ms），连跑 3 次。3000 条命令事件 37 ms 送达正常读取的订阅者；一个不读的订阅者在场时后台请求照常，它读回来时收到 `lagged`。
- 10 个订阅者（`ten_event_subscribers_cost_the_host_almost_nothing`，release，4 个 spinner 终端各 20 s）：后台 CPU 0.10% → 0.10%。
- 没验：App 的 GPU 基准没重跑（这一步只改后台和协议，App 没有新的行为）；10 分钟浸泡只跑了 20 秒的窗口；Codex 会话的事件只靠和 Claude 同一条状态判断，没单独测。
- 放弃条件的计时从今天起：两周内没有实际使用者就停在这一步。
