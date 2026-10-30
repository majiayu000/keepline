# Keepline 插件系统规格

状态：进行中（2026-10-30）。只针对 `next/` 原生 App。本文是总规格；agent 这一类插件的详细设计见 `docs/architecture/PLUGIN_SYSTEM.md`。

## 要支撑的决定

Keepline 的左栏、Agent 支持和窗格装饰，是否改成由插件提供，让别人不改源码也能加一种左栏、接一个新 Agent。

**验收（原型阶段，全部满足才继续）**

| 指标 | 门槛 | 怎么量 |
|---|---|---|
| App 体积增加 | ≤ 15 MB | release 构建，带插件运行时与不带对比 |
| 启动增加 | ≤ 100 ms | 运行时初始化 + 加载一个预编译插件，冷启动计时 |
| 一次左栏更新 | ≤ 2 ms（300 个会话） | 把 300 个会话交给插件、拿回左栏描述，取 p95 |
| 插件出错 | App 不崩，退回内置视图 | 故意写死循环、越界、返回乱数据的插件 |

**放弃条件**：任一门槛超标且没有明显的压缩办法时，改用「配置文件定制左栏」方案（`sidebar.toml` 写分组、折叠、字段、排序），不引入插件运行时。

## 两个维度：插件扩展什么、用什么写

**类型**（扩展什么），每个插件可以同时提供几种：

| 类型 | 文件 | 状态 |
|---|---|---|
| agent | `agent.json`、`agents/*.json` | 已完成（注册表、声明式 Agent、边界测试） |
| actions | `actions.json`：右键菜单与 ⌘P 的命令 | 已完成 |
| rules | `rules.json`：追加哪些命令算跑过验证 | 已完成 |
| sidebar | `sidebar.json`：左栏预设（经典 / 状态 / 目录为内置） | 已完成（声明式） |
| hooks / notifier | 生命周期事件交给命令或 webhook，通知出口 | 计划 |

**写法**（用什么写）：

| 写法 | 适合 | 例子 |
|---|---|---|
| 声明式（JSON） | 默认，最安全，不执行第三方代码 | 第三方 Agent、动作、规则、左栏预设 |
| WASM（沙盒） | 声明式表达不了的逻辑 | 自定义左栏排法、复杂对话记录解析 |
| 内置 Rust | 需要深度集成 | Claude / Codex / Grok 适配器（官方状态、子 Agent、共享后台） |

## agent 类型：一个 JSON 能做到什么

api 1 的声明能认出进程、启动和恢复、按 JSONL 绑定会话、卡片显示对话；回合状态、证据、标题都做不到，鸟瞰和左栏里分不准组。api 2 补上三项（都向下兼容，api 1 文件照常加载）：

| 能力 | 声明 | 验收用的真实 CLI |
|---|---|---|
| 回合状态与证据 | `transcripts.events`：`when` 用 JSON 指针匹配记录（值写 `"*"` 表示字段存在），命中后映射成回合开始 / 结束 / 等你回答 / 出错、工具调用、执行结果（含退出码），交给后台已有的 `phase` / `tool` / `result` | OpenCode |
| 会话标题 | `transcripts.title`：记录里的字段；SQLite 来源由 `sessions` 查询带 `_title`。索引文件来源未做，暂无需要它的 CLI | OpenCode |
| 对话来源：SQLite | `source: "sqlite"`：库路径、「哪些会话变了」的查询、「按会话增量读消息」的查询；只读打开、连接常驻、等锁很短、出错丢连接下次重开 | OpenCode |

仍需要内置 Rust 适配器的：官方状态查询、子 Agent、共享后台的画面关联、Hook 安装。会话命名分支（`feat/session-naming-titles`）给三个内置 Agent 加的标题读法留在各自适配器里，经同一个 `title()` 入口。

## 插件包与统一报告

```
~/.config/keepline/plugins/<id>/       # 遵循 XDG_CONFIG_HOME
  plugin.json   # {"api":1,"id","name","version?","description?","provides":["agent", …]}
  agent.json / agents/*.json   # provides 里每种类型对应的文件
```

- 实现：`keepline-core/src/plugin.rs`（发现、校验、报告），`agent::install()` 启动时调用。
- 规则：`api` 不符、`plugin.json` 缺失或格式不对、id 不合法或重名、声明了类型却没有文件，都只拒绝这一个插件并写明原因，其余照常加载。
- 旧位置 `~/.config/keepline/agents/*.json` 继续有效，和插件里的 Agent 声明进同一个注册表。
- 设置页「插件」一页列出每个插件和每个声明文件：已加载 / 没有加载、生效 / 没有生效及原因。

## 分层：哪些能插，哪些不能

| 层 | 是否插件 | 原因 |
|---|---|---|
| 终端渲染、VT 解析、输入 | 否 | 全 App 最热的路径，跨沙盒边界会加延迟和拷贝 |
| PTY 与 shell 进程管理 | 否 | 需要开进程、发信号，沙盒必须禁止 |
| **左栏视图** | 是，第一个开放 | 数据简单、调用低频，用来检验插件机制 |
| **Agent 适配器** | 是，第二个开放 | 价值最大；边界已存在（`keeplined/src/agents/mod.rs` 的 `Adapter`） |
| 终端事件钩子 | 是，之后 | 命令结束、出错、一轮结束这类低频事件 |
| 窗格标题 / 底栏装饰 | 是，之后 | 与左栏同一套描述 → 宿主绘制 |
| 主题配色 | 配置，不需要代码 | 已有 token 体系 |

## 架构

- **运行时**：wasmtime（与 Zed 扩展同一选择）。插件是 `.wasm`，在沙盒里跑：默认无文件、无网络、无进程能力；每次调用限时（fuel / epoch 中断）、限内存。
- **宿主位置**：
  - UI 类插件（左栏、窗格装饰）由 `keepline-app` 加载。
  - Agent 适配器、事件钩子由 `keeplined` 加载，因为数据在后台。
- **WASM 只在需要逻辑时用**：声明式能说清的（含经典 / 状态 / 目录三种左栏）一律用声明式；WASM 运行时（`crates/keepline-plugin-host`）保留为逃生口。
- **插件不画界面**：插件返回一份声明式描述，宿主用 Keepline 的主题画。A / C 样式、深浅色、无障碍标签都由宿主统一处理。
- **调用时机**：只在相关状态变化时调用，100 ms 内的多次变化合成一次；每秒变化的东西（如「在跑 3:12」计时）由插件声明，宿主自己刷新，插件不参与。

## 左栏插件接口（第一版）

输入（宿主 → 插件），只给左栏需要的字段：

```
State {
  sessions: [{ id, title, agent, group, cwd, space, updated, running_since?,
               stopped?, done?, needs_you, closed, selected }],
  spaces:   [{ id, name, path, missing }],
  tasks:    [{ id, title, started, done }],
  now
}
```

`group` 用宿主现有的分组判断（`session_group`），保证所有视图对「需要你」的判定一致。

输出（插件 → 宿主）：

```
Sidebar {
  sections: [{ id, title?, count?, folded_default, pinned,
               rows: [Row], summary? }]
}
Row = Session { id, glyph: status, title, trailing: time | timer | text,
                detail?: text, detail_mode: always | hover | selected, indent }
    | Folder  { id, name, path_on_hover, flag?, count, folded }
    | Note    { text, action? }
```

`action` 只能是宿主定义的少数几种：打开会话、展开 / 折叠分组、打开鸟瞰、打开 ⌘P。插件不能执行任意动作。

**权限**：原话全文、证据详情不在默认输入里；插件要用需在 `plugin.toml` 声明 `permissions = ["transcript"]`，用户首次启用时确认。

## 步骤

| # | 内容 | 状态 |
|---|---|---|
| 0 | 合入 agent 插件类型（注册表、声明式 Agent、适配器钩子、边界测试） | 完成（a1b8b60） |
| 1 | 插件包、统一报告、设置页「插件」 | 完成 |
| 2a | agent api 2：events（回合与证据）、SQLite 来源 | 完成。OpenCode 声明式插件（`next/plugins/opencode`）用本机真实库验收：近 30 天 403 个会话转写 2.4 MB、全部解析成卡片（377 做完了 / 26 在跑），证据 74 条（命令 53、改文件 18、出错 3） |
| 2b | 合入会话命名分支（命名规则、Claude/Grok/Codex 标题、Cursor 内置适配器）；声明式 `transcripts.title`，SQLite 来源可带 `_title`，标题变了单独写一条 | 完成。OpenCode 实测 403 个会话中 283 个用上自己的标题，其余（占位名 New session）退回首句 |
| 2 | actions：右键菜单与 ⌘P 共用的动作声明（`actions.json`：模板 `{cwd}` `{session_id}` `{transcript}` `{task}` `{agent}` 一律加引号；run = split / tab / paste；when = always / agent / shell / task） | 完成。实测：右键菜单「插件」分组、⌘P「动作」分组、缺变量时报错不执行；分屏命令在真实数据副本里执行并写出标记文件，目录正确。示例 `next/plugins/common-actions` |
| 3 | rules：验证命令规则，修掉 bun / vitest 项目被判成没跑验证 | 完成。内置补上 vitest/jest/nextest/uv/deno/swift/make 等和 `test:*` 脚本；`&&` 链任一段算、`;` 只认最后一段、`2>&1` 不影响、管道和后台不算；插件 `rules.json` 只能追加（`{"api":1,"verify":[["just","test"],["make","*"]]}`，按词前缀、比较路径末段，`*` 不能开头）。解析版本升到 12，近期会话重算一次。插件规则改了不触发重算，下次有新记录才按新规则判断 |
| 4 | sidebar：声明格式，经典 / 状态 / 目录三个内置预设，左栏顶部切换，⌘B 收起，按左栏减密稿 | 完成（声明式）。`{"api":1,"presets":[{"id","name","group":"flat|status|directory","fold":["running|done|stopped|stale"],"detail":"never|attention|always","limit"}]}`；需要你不能折叠，插件只能追加不能覆盖内置。状态：需要你、在跑摊开，其余合成一行；目录：需要你置顶不重复，目录名旁标「n 需要你」，没在跑的目录默认折叠，路径悬停显示。选择存 `~/.config/keepline/sidebar`。收起后留一条窄栏和需要你的数字。示例插件 `plugins/focus-sidebar`。截图核过四种状态（隔离 fixture）。WASM 排法与主界面 A/C 样式未做 |
| 5 | hooks / notifier：生命周期事件交给命令或 webhook，只读、带超时、失败不影响主流程 | |
| 6 | 有需求再做：功能开关、bash / fish 的 shell 集成、WASM 左栏接入 | |

每一步的验收：已有测试全过，真实数据副本截图对照原来的界面和性能不退步。

## 原型结果（2026-10-30）

结论：**四项门槛全部达标，继续走插件方案**（第 3 步起）。

| 指标 | 门槛 | 实测 | 结果 |
|---|---|---|---|
| 体积增加 | ≤ 15 MB | 约 8.1 MB（去掉符号后：带运行时 8.52 MB，对照程序 0.41 MB） | 达标 |
| 启动增加 | ≤ 100 ms | 运行时初始化 0.8 ms + 读预编译插件并实例化 1.1 ms；首次编译 32 ms，之后读磁盘缓存 | 达标 |
| 左栏更新（300 会话） | ≤ 2 ms | p50 0.65 ms，p95 0.76 ms，最大 0.98 ms | 达标 |
| 插件出错 | 不崩、可退回 | 死循环约 55 ms 被打断；乱数据回错误对象；之后新实例正常渲染 | 达标 |

- 环境：Apple Silicon，release 构建，wasmtime 50（仅 Cranelift + runtime，无 WASI、无组件模型），插件 82 KB（预编译后 245 KB）。
- 原型用 JSON 跨边界（`crates/keepline-plugin-host`，插件 `plugins/sidebar-status`，测量程序 `cargo run --release -p keepline-plugin-host --example bench -- <plugin.wasm> [会话数]`）。
- 1000 个会话时 p95 2.4 ms，超过 2 ms。耗时主要在 JSON 序列化：输入 365 KB、输出 230 KB。正式版应只传未关闭的会话（已关闭的只给数量），并在状态只变一两个会话时传增量。
- 体积数字是独立程序对比，不是 App 实际增量；App 里的真实增量在第 5 步接入时再量一次。
- wasmtime 50 需要 rustc ≥ 1.97，仓库工具链因此从 1.95.0 升到 1.99.1。
