# next/ 架构诊断与拆解（2026-10-31）

范围：Rust 新版 `next/crates/`（keepline-core、keeplined、keepline-app）。旧 TS 版 `src/` 只记结论，不在本轮改动。

## 本轮要支撑的决定

- 决定：keeplined 能否在**不改协议、不改数据库结构、不改行为**的前提下，把两个超大文件拆成职责单一的模块。
- 指标：
  1. 拆解前后 keepline-core、keepline-app、keeplined 的测试数一致且全部通过。
  2. `host.rs`（2813 行）和 `content.rs`（1999 行）拆成目录模块，生产代码单文件不超过约 900 行。
  3. 新旧文件排序后逐行比对，差异只有模块声明、再导出、文件说明和 `pub(super)`。
- 放弃条件：某一步需要改 `protocol.rs`、SQLite 表结构或测试断言才能通过，就回滚该步。

## 和 10-29 那一版的关系

10-29 在 `refactor/arch-decouple` 上做过四步拆解，没合进 main。之后 main 进了 95 个提交，其中：

- `Agent::all()` / `Agent::from_executable` 和按 Agent 分文件的 `keeplined/src/agents/` 已经由 Agent 注册表那批提交实现，第 1、2 步不再需要。
- `host.rs` 改了 842 行、`content.rs` 改了 506 行，旧的搬移提交无法 rebase，所以第 3、4 步在新 main 上按同样的分组重新搬了一次。

## 现状

### 依赖方向（健康）

```
keepline-app ──┐
               ├──> keepline-core (model / protocol / terminal / layout / content / agent)
keeplined ─────┘
```

- app 不依赖 daemon，两边只通过 `protocol::Request/Reply` 和 Unix socket 对话。
- keeplined 内部模块图无环，`host/` 是汇聚点；各 Agent 的识别和转录格式在 `agents/`。

### 问题清单

| # | 位置 | 问题 | 本轮处理 |
|---|---|---|---|
| 1 | `keeplined/src/host.rs` | 2813 行；终端 IO、日志关联、识别状态、客户端请求分发都在一个文件 | 拆成 `host/` 目录模块（纯搬移） |
| 2 | `keeplined/src/content.rs` | 1999 行，混了文本规则、转录解析 `Parser`、SQLite 读写 | 拆成 `content/{mod,rules,db}.rs` |
| 3 | `Host::snapshot()` / `Reply::Workspace` | 每次请求整份克隆 `Workspace` 回传，会话多时内存和 socket 流量线性增长 | 只记录，先实测 |
| 4 | 全局锁 | SQLite 写入、各类 pid 表更新都在同一把 `Arc<Mutex<Host>>` 下 | 拆锁是行为变化，先测锁等待时长 |
| 5 | app 大文件 | `terminal/pane.rs`、`workbench/` 下若干文件 | 其他会话正在改，本轮不动 |
| 6 | 三套实现并存 | `src/`（TS）、`next/`（Rust）、`menubar-tauri/` | 需要产品层决定旧版下线节奏 |

## 结果

决定：**采用**。两步都没改协议、表结构和测试断言，没有触发放弃条件。

| 提交 | 内容 |
|---|---|
| `refactor(keeplined): host.rs …` | `host/{mod,live,association,status,client}.rs` |
| `refactor(keeplined): content …` | `content/{mod,rules,db}.rs` |

拆解后的规模（生产代码，不含文件末尾测试）：

| 文件 | 职责 | 行数 |
|---|---|---|
| host/mod.rs | Host 状态、启动、接管旧 host、截止时间线程 | 710 |
| host/client.rs | 连接处理、请求分发、输入文本、交接 socket | 617 |
| host/association.rs | 日志事件订阅、补漏、日志与终端的关联 | 571 |
| host/live.rs | PTY 与订阅者、读泵、attach、退出回收 | 508 |
| host/status.rs | 识别状态推送、进程与注册表刷新、屏幕提问、官方状态对账、提交后探测 | 339 |
| content/mod.rs | `Parser` 与 hook 事实 | 670 |
| content/rules.rs | 完成声明、用户文本清理、验证命令、git commit | 389 |
| content/db.rs | 会话卡片、子代理关联、解析状态的读写 | 132 |

测试（拆解前后相同）：

| 套件 | 结果 |
|---|---|
| keepline-app 单元 | 78 过 |
| keepline-core 单元 | 62 过 |
| keeplined 单元 | 92 过 / 1 忽略 |
| keeplined architecture_boundary / declared_agent | 2 过；2 过 / 1 忽略 |
| managed_session 集成，默认并行 | 49 过 / 4 忽略 |

另外：`cargo clippy -p keeplined -p keepline-core --all-targets` 无警告；`--features visual-test` 的 app 能编译；原生场景 `KEEPLINE_SPACE_EVIDENCE` 用拆分后的 keeplined 跑通（起终端、空间改名并存住）。

## 后续（本轮未做）

1. `dispatch`（client.rs）仍是一个处理全部请求的大 match，可以按请求类别拆成几个函数。
2. 全局锁拆细、`Workspace` 改成只发变化的部分：先测锁等待和每次回传的字节数，再决定做不做。
3. app 侧大文件：等正在改它们的会话合入后再动。
