<div align="center">

<img src="docs/assets/logo.svg" alt="Keepline Logo" width="120" />

# Keepline

### 看清 agent 做了什么，还有什么需要你处理

**本地运行的 Codex / Claude Code 进度账本。对照你的要求与执行证据，由你明确验收结果。**

[![GitHub stars](https://img.shields.io/github/stars/majiayu000/keepline?style=flat-square&color=ffcc00)](https://github.com/majiayu000/keepline)
[![License](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](LICENSE)

[English](README.md) | 中文

[快速开始](#快速开始) | [功能特性](#功能特性) | [主界面](#web-仪表板) | [CLI 命令](#cli-命令)

<br />


</div>

---

## Keepline 帮你判断什么

离开一会儿再回来，先看哪个 Agent 需要回答、改了什么、跑过哪些检查，
再核对它是否满足要求，回到原会话继续处理。工作完成由你明确验收。

Keepline 正在转向原生 Agent 工作台：Rust 后台进程加 GPUI 客户端，每个会话是 App 内的真实终端，
见[产品卡](https://github.com/majiayu000/keepline/blob/feat/agent-workbench/specs/product-card.md)和 [#147](https://github.com/majiayu000/keepline/pull/147)。合入之前，本分支的 Bun CLI、浏览器界面和 Tauri 菜单栏
继续提供会话监控和恢复入口；1.2.0 菜单栏候选不计划发布。

## 进度账本

- **要求**：保留用户原话，列出可核对的要求条目。
- **证据**：关联记录中的命令结果、测试输出、文件操作和提交。Agent 自述完成不算执行证据。
- **验收**：展示缺口与计划外操作，仅用户能明确验收。
- **目标与待办**：关联会话，逐级汇总进度。
- **进程与回合状态**：分别判断。没有观察到进程时保留未知状态，不能据此断定退出或完成。

默认读取本地会话记录，不装 Hook 也能工作。Hook 用于接收实时事件；
模型分析另行配置，用于辅助核对要求和证据，不能编造证据。
详见[产品规范](specs/progress-ledger/product.md)和[技术设计](specs/progress-ledger/tech.md)。

## 仓库范围

- `src/`：Bun 服务、CLI、会话适配器、进度账本与 React 主界面。
- `menubar-tauri/`：macOS 桌面应用、内嵌服务、托盘与通知。
- `sdk/swift/`：稳定本地 API 的客户端。
- `next/crates/keeplined/`：实验性 Rust PTY 守护进程。
- [`specs/terminal-workbench/`](https://github.com/majiayu000/keepline/tree/feat/agent-workbench/specs/terminal-workbench)（位于 `feat/agent-workbench` 分支）：原生 Agent 工作台的规格，在该分支实现，尚未合入。

---

## 快速开始

### 从源码构建 Bun CLI 和仪表板

需要 Git 和 Bun 1.1+。

```bash
git clone https://github.com/majiayu000/keepline.git keepline
cd keepline
bun install --frozen-lockfile
(cd src/web/client && bun install --frozen-lockfile)
bun run build
bun run start web
```

打开 **http://127.0.0.1:3377**。使用源码 checkout 时，下面的 CLI 示例中，
请在本仓库目录用 `bun dist/index.js` 代替 `keepline`。

默认只绑定本机回环地址；如果你确实要对外暴露，再设置 `KEEPLINE_HOST`。

### Keepline 桌面候选（1.2.0）

当前源码提供菜单栏面板和进度账，内置本地 Bun 服务，无需另外安装 CLI 才能打开面板。
**当前是候选版本，不能据此认为 1.2.0 已公开发布；它也不计划发布，菜单栏将由原生工作台承担。** Releases 中已有安装包可能仍是旧版额度应用。
构建命令见[桌面说明](menubar-tauri/README.md)。

1. 打开 Keepline，等待首次本地扫描结束。
2. 进入「设置 → 连接 agent」，点击「连接实时事件」。这会安装 Keepline Hook，保留已有命令。
3. 在 Codex 中信任新 Hook，或重新打开 Claude Code，然后运行一个回合。
4. 确认状态显示「已收到真实事件」。仅「配置已安装」不能证明连接成功；不安装 Hook 也能定时读取本地执行记录。
5. 打开会话，核对要求、执行证据和缺口，再由你明确验收。

---

## 功能特性

### 进度账与菜单栏

- 对照用户要求、执行步骤、测试证据和明确验收，保留未确认的候选要求。
- 将会话关联到目标与待办，生成追问，恢复中断会话。
- 区分进程状态与回合状态：扫描不到不等于已退出。
- 菜单栏展示待处理数量、会话入口和专注模式；系统通知默认关闭。
- 默认解析本地执行记录；模型分析需另外配置。


### 实时会话监控

监控系统上所有 Codex 和 Claude Code 会话。状态、当前文件、最后工具、活跃度一目了然。

```
┌─────────────────────────────────────────────────────────────────┐
│ 会话列表                                            3 个活跃    │
├─────────────────────────────────────────────────────────────────┤
│ ● 运行中   my-app          编辑: src/auth.ts      2秒前       │
│ ● 等待中   api-service     读取: README.md        5分钟前     │
│ ○ 空闲     docs            写入: guide.md        15分钟前     │
│ ✗ 丢失     old-project     —                      2小时前     │
└─────────────────────────────────────────────────────────────────┘
```

### 一键会话恢复

终端退出后，先定位被标记为 lost 的会话，再选择该会话可用的恢复方式。恢复依赖本地文件、项目目录和原 CLI 的能力。

```bash
keepline recover <session-id>
```

三种恢复方式：
- **Resume（恢复）** — 有对应会话文件时，用原 CLI 恢复指定会话
- **Continue（继续）** — 用原 CLI 继续该目录的最近会话（可能不是指定 ID）
- **New（新建）** — 用原始 prompt 全新开始

### 找回中断的 Codex 或 Claude Code 会话

```bash
keepline list --status lost
keepline recover
keepline recover <session-id> --method resume
```

第一条命令列出 lost 会话；不带参数的 `recover` 显示恢复候选与可用方法。
优先使用完整 session ID，确认项目目录和 CLI，再选择 `resume`、`continue` 或 `new`。
不带 `--terminal` 时，命令显示需要你在终端执行的恢复命令；`--terminal` 会尝试打开终端执行。
运行中或已完成的会话不属于 lost 恢复范围，目录不存在也会报错。

`resume` 不会重建已经被删除的 transcript，`new` 只以原始 prompt 开始新会话。
需要加密备份和导出恢复演练时，参见 [chat-archive-rs](https://github.com/majiayu000/chat-archive-rs)；
Keepline 的监控和运行时恢复不等于归档备份。


### 用量、费用与跨会话记忆

按各运行时实际提供的数据统计 token，记录中有缓存读写时计入对应费用。
费用由模型价格表估算，不能据此确认供应商账单或套餐额度，见[计价实现](src/services/usage.pricing.ts)。

记忆命令保留会话笔记与恢复上下文，独立于账本中的执行证据和用户验收。

### Plans 追踪

追踪 Claude 的实施计划。查看阶段、任务和完成率。

```
┌─────────────────────────────────────────────────────────────────┐
│ 计划列表                                                        │
├─────────────────────────────────────────────────────────────────┤
│ Auth 系统重构                                     ████████░░ 80% │
│   Phase 1: 数据库 Schema                          ✓ 已完成      │
│   Phase 2: API 端点                               ✓ 已完成      │
│   Phase 3: 前端集成                               ● 进行中      │
│   Phase 4: 测试                                   ○ 待开始      │
└─────────────────────────────────────────────────────────────────┘
```

### Web 仪表板

主界面为进度账本，包含总览、待办、目标与回顾。
会话记录、计划、记忆与用量统计从更多导航进入；桌面 App 使用同一套主界面。

`/` 聚焦搜索，`Cmd+,` 或 `Ctrl+,` 打开设置。
[`docs/assets/`](docs/assets/) 中的旧截图保留作历史参考，展示的是早期监控面板。

---

## CLI 命令

```bash
# 核心命令
keepline                      # 启动 Web 仪表板（默认）
keepline list                 # 列出所有会话
keepline watch                # 终端实时监控
keepline recover <id>         # 恢复丢失的会话

# 会话管理
keepline list -s running      # 按状态过滤
keepline list -d ./my-app     # 按目录过滤

# 记忆管理
keepline memory list          # 列出会话记忆
keepline memory show <id>     # 显示记忆详情
keepline memory export <id>   # 导出恢复上下文

# 后台服务
keepline daemon start         # 启动后台监控
keepline daemon stop          # 停止守护进程
keepline hooks install        # 安装 Claude 兼容 hooks
```

`keepline web` 只启动 dashboard。`keepline daemon start` 才会运行后台扫描器和
hook receiver。`keepline hooks install` 只安装转发命令；用 `keepline status`
或 `keepline hooks status` 检查 receiver 是否在运行。

---

## 工作方式

1. 扫描 Codex / Claude Code 进程，并解析本地会话记录。
2. 聚合进程观察与回合活动；启动时完成对账后再开放业务路由。
3. 账本关联要求与执行证据，SQLite 保存本地状态和用户验收。
4. Hono API 与 WebSocket 向 React 界面和桌面 App 提供数据，Hook 可补充实时事件。

[分层依赖测试](src/__tests__/architecture-imports.test.ts)约束模块边界。
`/api/v1` 是 Swift SDK 使用的稳定本地接口，恢复部分有 [OpenAPI 契约](docs/local-api-v1-recovery.openapi.yaml)，
`/api` 同时为 Web 界面提供接口。

---

## 配置

Keepline 默认把数据存储在 `~/.keepline/`：

```
~/.keepline/
├── keepline.db    # SQLite 数据库
├── config.json      # 配置文件
└── keepline.log   # 日志文件
```

### 选项

```json
{
  "scanInterval": 5000,
  "hookPort": 7890,
  "webPort": 3377,
  "logLevel": "info"
}
```

描述进程或安全边界的环境变量，例如 `KEEPLINE_HOME`、`KEEPLINE_HOST`、
`KEEPLINE_PUBLIC_ORIGIN`、`KEEPLINE_ALLOWED_ORIGINS` 和
`KEEPLINE_TRUST_PROXY`，仍由对应运行时边界读取，不写入 `config.json`。

---

## 开发

```bash
bun run dev          # 开发模式
bun run typecheck    # 类型检查
bun run test         # 使用隔离数据目录运行后端测试
```

---

## 下一步与发布状态

源码包、CLI、本地服务与桌面候选统一报告 **1.2.0**。
该版本号仅表示源码版本。正式签名、公证、公开发布与原生人工验收各有独立证据，
请查看[验收记录](specs/progress-ledger/verification.md)和[桌面构建说明](menubar-tauri/README.md)。
[10-19 维护记录](specs/progress-ledger/maintenance-20261019.md)归档本轮源码同步、检查结果与未完成的发布门槛。

下一步是[原生 Agent 工作台](https://github.com/majiayu000/keepline/tree/feat/agent-workbench/specs/terminal-workbench)：在 App 内启动、切换真实 Agent CLI，回答请求并继续工作。
它在 `feat/agent-workbench` 分支（[#147](https://github.com/majiayu000/keepline/pull/147)）开发，不属于本分支。

---

<div align="center">

**为 Codex 和 Claude Code 重度用户打造**

MIT License

</div>
