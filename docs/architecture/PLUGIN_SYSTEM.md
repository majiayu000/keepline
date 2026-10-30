# Keepline 插件系统设计（next/，2026-10-30）

范围：Rust 新版 `next/crates/`。参考了 magpie（`internal/pluginrt`、`internal/agent`、`*.agent.js`）和 DeepSeek Harness（cordis 插件树、bundle + patch 分层、按服务可用性激活）。

## 要支撑的决定

- 决定：Keepline 的核心（daemon 的 host / 识别 / 日志索引，App 的界面）能否做到**不认识任何具体 Agent**。新增一个 Agent 时，只需要加一份描述，加一个适配器或者写一个声明文件，核心代码一行都不用改。
- 指标：
  1. `agents/` 目录和 `agent.rs` 内置清单之外，生产代码里不出现 `Agent::Claude`、`Agent::Codex`、`Agent::Grok`，由边界测试守住（`architecture_boundary`）。
  2. 写一个 `~/.config/keepline/agents/<id>.json`、不重新编译，就能让一个新 CLI 被识别并显示在设置里。如果它写对话记录，还能和画面绑定，用上「停止 Agent」「挂到待办」「取消关联」。这条由集成测试验证。
  3. 数据库和协议的取值不变（`"Codex"` / `"Claude"` / `"Grok"` 原样存取），已有测试全部通过。
- 放弃条件：如果为了让 Agent 支持动态加载，必须改 SQLite 表结构或协议 `MAJOR`，或者让现有测试的断言变弱，就退回到「仍用枚举，只把 host 里的特判收进适配器」这一步，声明式 Agent 留到下一轮。

## 两个参考对象各借了什么

| | magpie | dsh（cordis） | Keepline 取舍 |
|---|---|---|---|
| Agent 的身份 | `Agent` 是结构体（数据）加可选钩子；插件里的 Agent 用 `*.agent.js` 声明 | 无 | **采用**：Agent 改成可以动态注册的标识加描述，钩子留空就是不支持 |
| 功能插件 | `pluginrt.Register(Manifest)`，带 `Requires/Provides`、`InUse`、开关和故障隔离 | 插件行 `{id, name, config, disabled}`，由所需服务是否可用决定激活 | **先只设计**，见「第二阶段」 |
| 扩展点 | observer / strategy / middleware / tool / budget，每个点各自规定失败时怎么办（放行或拒绝） | seam：同一个服务可以换实现（`dsh-bash-local` 和 `dsh-bash-sandbox`） | **采用**：每个 seam 写清失败策略 |
| 组合与覆盖 | 默认值 → 预设 → 用户设置 → 正在使用 | 多个 bundle 的 patch 叠加，再叠用户 patch，同一 id 最后写入的生效 | **采用**：内置值 → 用户文件，按 id 覆盖 |
| 边界守护 | `TestPluginsImportOnlyCore` | 包之间靠服务注入，不直接 import | **采用**：Rust 侧加边界测试 |
| 版本 | 清单写 `API: "^1"`，不匹配就标 failed | 包的 semver | **采用**：清单带 `api`，不兼容就拒绝加载并说明原因 |

没有照搬的部分：magpie 的 JS 宿主（moejs / Bun）、iframe 页面插件，以及 dsh 的 `!!js` 表达式。Keepline 是一个 Rust 二进制，第三方代码执行目前没有需求，声明式描述已经够用。

## 概念

```
                  ┌──────────── keepline-core::agent ─────────────┐
 内置描述 ──┐      │  Agent（可复制的标识，序列化为键）              │
            ├──►  │  AgentSpec（名字、可执行文件、启动/恢复模板、   │ ◄── App 只读这里
 声明文件 ──┘      │            对话记录的声明）                     │
 ~/.config/        │  Registry：按 id 合并，后写入的覆盖先写入的     │
 keepline/agents/  └────────────────────────────────────────────────┘
                                   │
                  ┌─ keeplined::agents（daemon 的 seam）─┐
                  │  trait Adapter                         │
                  │   识别：runs / process_session         │
                  │   记录：root / identity / add …        │
                  │   生命周期：pending / title_status     │
                  │   状态源：official                      │
                  │   关联：outside_pty                     │
                  │  内置实现：claude / codex / grok       │
                  │  声明实现：Declared(AgentSpec)          │
                  └────────────────────────────────────────┘
                                   │
                  host / recognition / log_index：只调用 trait，不问「你是谁」
```

### 1. Agent 标识（core）

- `Agent` 是一个可复制的标识，内部存的是一个键。内置三个常量 `Agent::Codex`、`Agent::Claude`、`Agent::Grok`，它们的键就是现在存进库里的 `"Codex"`、`"Claude"`、`"Grok"`，所以数据库和协议的取值不变。
- 声明式 Agent 的键就是它的 `id`。这个 id 不能和内置键重名（不区分大小写），也不能和别的声明重名。
- 从库里读出一个当前注册表里没有的键时（比如对应的插件被删了），仍然能还原出一个 `Agent`：它显示为这个键本身，没有适配器，不能启动。历史会话照常显示，不会因此读库失败。
- `AgentSpec` 是纯数据：名字、可执行文件、解释器脚本后缀、启动和恢复命令模板（`{id}` 换成加了引号的会话 ID，`{uuid}` 换成新生成的 UUID），以及可选的对话记录声明。App 和 daemon 都从 core 读它，不经过协议传递。

### 2. 注册表与组合

- 加载顺序：先放内置描述，再读 `~/.config/keepline/agents/*.json`（遵循 `XDG_CONFIG_HOME`），按文件名排序依次读入。
- 覆盖规则参考 dsh 的 patch：**同一 id 后写入的覆盖先写入的**，但只允许覆盖声明式的条目。一个文件如果想改写内置 Agent，会被拒绝，并在加载报告里说明原因。内置 Agent 的行为写在代码里，用声明覆盖一半只会造成半生效的状态。
- 加载失败不会中断启动。每个文件的结果（loaded / rejected 及原因）都记在 `Registry::report()` 里，供设置页和诊断面板读取。这对应 magpie 的 `failed` 状态：一个插件坏了，其他照常加载。
- App 和 daemon 在启动时各加载一次（`agent::install`）。测试不调用它，就只有内置的三个。

### 3. Adapter seam（daemon）

现有的 `Adapter` trait 再加四个钩子，把 host.rs 里写死的 Claude / Codex 分支搬进来：

| 钩子 | 默认 | 谁实现 | 取代的 host 特判 |
|---|---|---|---|
| `pending(process, processes) -> Pending` | `Wait`：等对话记录出现 | Claude：子会话不写记录，返回 `Unrecorded(提示)`；Codex：共享 daemon 模式下返回 `Unbound{title_status}` | `host.rs` 识别循环里的两个分支 |
| `title_status(title) -> Option<String>` | `None` | Codex：从标题读出「工作中 / 需要你处理 / 空闲」 | `codex_hint` |
| `outside_pty() -> bool` | `false` | Codex：会话跑在 PTY 外，靠画面关联、跟踪打开着的日志 | `associate_screens` 和 `follow_associated` 的 `== Codex` |
| `official(due) -> Option<OfficialResult>` | `None` | Claude：`claude agents --json`；Codex：app-server 的 `thread/read` | `apply_official` 的两段 |

子 Agent 的日志由现有的 `links_children()` 决定，也取代 `follow_associated` 里的 `== Codex`。

每个 seam 的失败策略都要写明：

- `official` **失败时放行**：查不到官方状态，就退回到只看日志和画面，并在识别提示里说明。行为和现在一样。
- 「停止 Agent」**失败时拒绝**：PID、运行时、所属终端三项当场核对，有一项对不上就不发信号。这一条不因插件化而放宽，声明式 Agent 走同一套核对。
- 声明式 Agent 的记录解析**跳过坏记录**：某条记录不符合声明就略过它，不让整个文件读取失败。

### 4. 声明式 Agent（第三方，不需要重新编译）

`~/.config/keepline/agents/aider.json`：

```json
{
  "api": 1,
  "id": "aider",
  "name": "Aider",
  "executable": "aider",
  "scripts": ["/bin/aider"],
  "start": "aider",
  "resume": "aider --restore-chat-history",
  "transcripts": {
    "root": "~/.aider/sessions",
    "extension": "jsonl",
    "session": "/session_id",
    "cwd": "/cwd",
    "role": "/role",
    "user": "user",
    "assistant": "assistant",
    "text": "/content"
  }
}
```

- `api` 写的是声明格式的主版本，现在是 `1`。比它新的版本会被拒绝并说明原因。
- 识别规则：可执行文件名等于 `executable`；或者进程是解释器（`node`、`bun`、`python*`），而脚本路径以 `scripts` 里某一项结尾。
- 没有写 `transcripts` 的 Agent 能被识别，画面上会提示「已识别 X，它不写对话记录，所以不能关联待办」，但不会绑定会话。这和 Claude 子会话走的是同一条 `Unrecorded` 路径。
- 写了 `transcripts` 的 Agent，在 `root` 下按扩展名找文件。每条记录用 JSON 指针读出会话 ID 和工作目录，用户和助手的消息写进卡片，所以绑定、停止、挂到待办都能用。回合状态声明式描述不了，只靠画面。
- 做不到的事要明说：官方状态、共享 daemon 下的画面关联、子 Agent 这些还是要写 Rust 适配器。

### 5. 边界

`keeplined/tests/architecture_boundary.rs` 扫描 `next/crates/*/src`，规则如下：

- 只有 `keeplined/src/agents/` 和 `keepline-core/src/agent.rs` 可以写 `Agent::Claude`、`Agent::Codex`、`Agent::Grok`。
- 测试代码不受限制：`#[cfg(test)]` 之后的部分、`tests/`、`examples/`。
- 违规时报出文件和行号，并提示把逻辑挪进对应 Agent 的适配器钩子。

## 第二阶段：功能插件（本轮只设计，不实现）

等真的出现「用户想关掉某个功能」或者「第三方想加一个面板」的需求时再做，形状先定下来，免得第一阶段的接口把路堵死：

- `Manifest { id, api, requires, provides, essential, in_use, apply }`。daemon 里的功能（官方状态轮询、额度、诊断、通知）和 App 里的视图（待办、概览、诊断面板）都写成这种形式。
- `Runtime::reconcile(on)`：先按反向顺序停掉不再需要的插件，再按服务依赖顺序启动其余插件。某个插件 `apply` 出错，只把它自己标成 failed。
- `~/.config/keepline/plugins.json`：每行是 `{id, enabled, config}`，只记录用户改过的部分。正在使用的功能（`in_use > 0`）不允许关掉。
- App 侧的槽位：侧栏页面、画面右键菜单项（类似 magpie P6 的 `actions`）、设置分区。插件出错时由错误边界替换成「这个部分出错了」的卡片，其他部分照常运行。

第一阶段为此留下的口子：`Registry::report()` 的形状可以直接复用为插件状态；声明文件的 `api` 字段和第二阶段共用同一种版本规则。

## 实施步骤（第一阶段）

1. core：`Agent` 从枚举改成标识，加上 `AgentSpec`、`Registry` 和 JSON 声明的加载器。键和序列化保持不变。设置页的默认 Agent 改成列出注册表里的全部，Grok 也能选了。
2. daemon：Adapter 加 `pending`、`title_status`、`outside_pty`、`official` 四个钩子，host 里的特判搬进 claude / codex 适配器。
3. daemon：实现 `Declared` 适配器，App 和 daemon 启动时调用 `agent::install`。
4. 边界测试，加上声明式 Agent 的集成测试（假 CLI 加假对话记录，覆盖识别、绑定和停止）。

## 结果（2026-10-30）

决定：**第一阶段采用**。三个指标都已达到，没有触发放弃条件：表结构没改，协议 `MAJOR` 没变，只把 `PROTOCOL_MINOR` 从 0 升到 1，新增的 `terminal_notes` 带默认值。现有测试的断言一条都没放宽。第二阶段（功能插件）**待定**，有真实需求再开工。

实际改动与设计的出入：

- 原来 App 拿到提示字符串后，自己猜是 Claude 还是 Codex 再翻译成说明文字（`layout.rs` 里的 `unread_agent`），这也是一处耦合。现在改成由适配器直接给出 `TerminalNote {note, why}`，经 `RecognitionStatus.terminal_notes` 发给 App，App 原样显示。`terminal_hints` 保留不变。
- 从索引库读回已经删掉声明的 Agent 时，不再报 `unknown indexed runtime`，而是还原成一个没有适配器的 Agent（`Missing`）。否则删掉一个声明文件，整个索引查询都会失败。
- 设置页的「默认 Agent」原计划改成 `Agent::all()`，但 `settings.rs` 是主工作区里另一个会话还没提交的文件，本分支里没有它。那边写死成了 `[Codex, Claude]`，要等它合入时再改。

测试（基线是 a392a03）：

| 套件 | 基线 | 本分支 |
|---|---|---|
| keepline-core 单元 | 37 过 | 43 过（新增注册表、声明加载、覆盖与拒绝） |
| keeplined 单元 | 52 过 / 1 忽略 | 58 过 / 1 忽略（新增声明式适配器、Codex 钩子、已删声明的索引行） |
| architecture_boundary | — | 2 过；在 `recognition.rs` 里人为塞一行 `Agent::Codex`，测试能报出文件和行号 |
| declared_agent 集成 | — | 2 过，连跑 3 次都稳定：只靠声明文件就完成识别、绑定、停止 Agent（Shell 保留）、库里存 `fake`；没写对话记录时在画面上提示，进程退出后提示消失 |
| managed_session 集成 | — | 40 过 / 1 失败。两轮失败的分别是 `a_dialog_gone…claude_official_status` 和 `forty_idle_terminals…`，单独跑各通过 5 次和 3 次。当时机器负载在 14 左右，这是 10-29 诊断里记过的计时敏感测试 |
| keepline-app | — | 56 过 |

已验：上面这些测试的输出，以及边界测试确实能抓到人为塞进去的违规。
没验：没有打包 App 实机看画面上的说明文字；没有拿真实的第三方 CLI（Aider 等）写声明来试，只用了 `/bin/sh` 冒充的假 CLI。
相关情况：Grok 没有官方状态钩子，也不走共享后台，行为不变，它的 Adapter 测试照常通过。Claude 子会话的提示由 `managed_session` 里的 `claude_without_a_transcript_says_so_until_it_exits` 覆盖，仍然通过。
