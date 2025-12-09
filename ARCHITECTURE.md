# Tasker - Claude Code 会话监控与恢复系统

## 架构概述

采用 **分层架构 + 领域驱动设计 (DDD)** 的混合模式。

```
┌─────────────────────────────────────────────────────────────────┐
│                        Presentation Layer                        │
│                      (CLI / TUI Commands)                        │
├─────────────────────────────────────────────────────────────────┤
│                       Application Layer                          │
│                   (Services / Use Cases)                         │
├─────────────────────────────────────────────────────────────────┤
│                        Domain Layer                              │
│              (Entities / Value Objects / Events)                 │
├─────────────────────────────────────────────────────────────────┤
│                      Infrastructure Layer                        │
│         (Database / Process Monitor / File Parsers)              │
└─────────────────────────────────────────────────────────────────┘
```

## 设计模式

1. **Repository Pattern** - 数据访问抽象
2. **Observer Pattern** - 事件驱动通信
3. **Factory Pattern** - 实体创建
4. **Strategy Pattern** - 状态检测策略
5. **Command Pattern** - CLI 命令执行
6. **Singleton Pattern** - 数据库连接、配置

## 文件结构

```
src/
├── index.ts                    # CLI 入口 (~50 行)
│
├── core/                       # 核心领域层
│   ├── index.ts               # 导出 (~15 行)
│   ├── types.ts               # 基础类型 (~80 行)
│   ├── errors.ts              # 自定义错误 (~60 行)
│   └── events.ts              # 领域事件 (~50 行)
│
├── session/                    # 会话领域
│   ├── index.ts               # 导出 (~20 行)
│   ├── types.ts               # 会话类型 (~70 行)
│   ├── entity.ts              # 会话实体 (~100 行)
│   ├── repository.ts          # 仓库接口 (~40 行)
│   ├── service.ts             # 会话服务 (~180 行)
│   └── aggregator.ts          # 状态聚合 (~120 行)
│
├── claude/                     # Claude 数据解析
│   ├── index.ts               # 导出 (~15 行)
│   ├── types.ts               # Claude 数据类型 (~120 行)
│   ├── parser/                # 解析器
│   │   ├── index.ts           # (~15 行)
│   │   ├── jsonl.ts           # JSONL 解析 (~150 行)
│   │   └── history.ts         # 历史解析 (~80 行)
│   └── scanner.ts             # 项目扫描 (~100 行)
│
├── process/                    # 进程监控
│   ├── index.ts               # 导出 (~15 行)
│   ├── types.ts               # 进程类型 (~50 行)
│   ├── scanner.ts             # 进程扫描 (~120 行)
│   └── detector.ts            # 状态检测 (~100 行)
│
├── storage/                    # 数据持久化
│   ├── index.ts               # 导出 (~15 行)
│   ├── database.ts            # 数据库管理 (~100 行)
│   ├── migrations.ts          # 迁移脚本 (~80 行)
│   └── session.repository.ts  # 会话仓库实现 (~180 行)
│
├── hook/                       # Hook 服务
│   ├── index.ts               # 导出 (~15 行)
│   ├── types.ts               # Hook 类型 (~60 行)
│   ├── server.ts              # HTTP 服务 (~120 行)
│   └── installer.ts           # Hook 安装 (~100 行)
│
├── recovery/                   # 恢复引擎
│   ├── index.ts               # 导出 (~15 行)
│   ├── types.ts               # 恢复类型 (~40 行)
│   ├── service.ts             # 恢复服务 (~150 行)
│   └── terminal.ts            # 终端操作 (~100 行)
│
├── daemon/                     # 守护进程
│   ├── index.ts               # 导出 (~15 行)
│   ├── manager.ts             # 进程管理 (~150 行)
│   └── scheduler.ts           # 调度器 (~100 行)
│
├── commands/                   # CLI 命令
│   ├── index.ts               # 命令注册 (~50 行)
│   ├── list.ts                # list 命令 (~100 行)
│   ├── watch.ts               # watch 命令 (~80 行)
│   ├── recover.ts             # recover 命令 (~100 行)
│   ├── daemon.ts              # daemon 命令 (~80 行)
│   └── status.ts              # status 命令 (~60 行)
│
└── utils/                      # 工具函数
    ├── index.ts               # 导出 (~15 行)
    ├── paths.ts               # 路径处理 (~60 行)
    ├── logger.ts              # 日志系统 (~80 行)
    ├── config.ts              # 配置管理 (~70 行)
    └── format.ts              # 格式化输出 (~80 行)
```

## 模块职责

### Core (核心)
- 定义全局类型和接口
- 自定义错误类
- 领域事件定义

### Session (会话)
- 会话实体和值对象
- 会话仓库抽象
- 会话服务（CRUD + 状态管理）
- 状态聚合器（合并进程状态和文件状态）

### Claude (Claude 数据)
- JSONL 文件解析
- 历史记录解析
- 项目目录扫描

### Process (进程)
- 系统进程扫描
- Claude 进程识别
- 工作目录获取

### Storage (存储)
- SQLite 数据库管理
- 会话仓库实现
- 数据迁移

### Hook (钩子)
- HTTP 服务接收事件
- Claude hooks 配置安装

### Recovery (恢复)
- 会话恢复策略
- 终端操作（打开新终端窗口）

### Daemon (守护进程)
- 后台进程管理
- 定时扫描调度

### Commands (命令)
- CLI 命令实现
- 参数解析
- 输出格式化

## 数据流

```
┌──────────────┐    扫描     ┌──────────────┐
│   Process    │ ────────> │              │
│   Scanner    │            │              │
└──────────────┘            │   Session    │    持久化    ┌──────────┐
                            │  Aggregator  │ ──────────> │  SQLite  │
┌──────────────┐    解析     │              │              └──────────┘
│    Claude    │ ────────> │              │
│    Parser    │            └──────────────┘
└──────────────┘                   │
                                   │ 事件
┌──────────────┐    事件           ▼
│     Hook     │ ────────> ┌──────────────┐
│    Server    │            │    Event     │
└──────────────┘            │   Emitter    │
                            └──────────────┘
                                   │
                    ┌──────────────┼──────────────┐
                    ▼              ▼              ▼
             ┌──────────┐  ┌──────────┐  ┌──────────┐
             │   CLI    │  │  Daemon  │  │ Recovery │
             └──────────┘  └──────────┘  └──────────┘
```

## 依赖关系

```
commands ──> session ──> storage
    │            │
    │            ├──> claude
    │            │
    │            └──> process
    │
    └──> recovery ──> session
             │
             └──> terminal

daemon ──> session
   │
   └──> hook

所有模块 ──> core, utils
```
