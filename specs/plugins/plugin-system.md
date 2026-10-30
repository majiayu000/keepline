# Keepline 插件系统规格

状态：草案（2026-10-30），原型待验证。只针对 `next/` 原生 App。

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

## 插件包

```
~/.keepline/plugins/<name>/
  plugin.toml   # name, version, kind = "sidebar", api = 1, permissions
  plugin.wasm
```

- 内置的「经典 / 状态 / 目录」三个视图也写成插件，打包时预编译进 App，用来检验接口。
- 文件变化时热重载；加载或运行出错，退回「经典」并提示插件名和错误。
- 提供 Rust 插件模板和 `keepline plugin new`；其他语言之后再说。

## 步骤

1. 本规格。
2. **原型**：独立 crate `keepline-plugin-host` 接入 wasmtime，一个最小左栏插件，按上表量体积、启动、300 会话更新耗时和出错退回。对照门槛做决定，结果写回本文。
3. 用 WIT 定义 `keepline:sidebar` 接口（原型阶段先用 JSON 跨边界，量的是最坏情况）。
4. 左栏抽出接口，把经典 / 状态 / 目录依次改成插件，逐个用真实数据副本截图核对。
5. 左栏顶部切换、设置项、插件目录、热重载、出错退回。
6. 模板、`keepline plugin new`、写插件说明。
7. 之后：Agent 适配器插件化（后台）、事件钩子、窗格装饰。

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
