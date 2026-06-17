# Keepline Architecture Design

> **目标**: 从「会话监控工具」升级为「Claude Code 自动化平台」
> **设计原则**: 接力赛模式 | 容错优先 | 人类在循环中 | 渐进增强

---

## 1. 系统架构总览

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           KEEPLINE V2 ARCHITECTURE                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         INTERFACE LAYER                              │   │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐   │   │
│  │  │   CLI   │  │ Web UI  │  │  Slack  │  │ Discord │  │ Webhook │   │   │
│  │  │ (Cmdr)  │  │ (React) │  │  Bot    │  │  Bot    │  │   API   │   │   │
│  │  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘   │   │
│  └───────┼────────────┼────────────┼────────────┼────────────┼─────────┘   │
│          │            │            │            │            │             │
│          └────────────┴────────────┼────────────┴────────────┘             │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      APPLICATION LAYER                               │   │
│  │                                                                      │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │   │
│  │  │  Use Cases   │  │  Workflows   │  │   Plugins    │               │   │
│  │  │              │  │              │  │              │               │   │
│  │  │ • List       │  │ • DailyAudit │  │ • Slack      │               │   │
│  │  │ • Recover    │  │ • AutoFix    │  │ • Metrics    │               │   │
│  │  │ • Schedule   │  │ • Migration  │  │ • Custom     │               │   │
│  │  │ • Watch      │  │              │  │              │               │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        DOMAIN LAYER                                  │   │
│  │                                                                      │   │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐       │   │
│  │  │  Session   │ │  Recovery  │ │   Task     │ │   Memory   │       │   │
│  │  │  Domain    │ │  Domain    │ │  Domain    │ │   Domain   │       │   │
│  │  │            │ │            │ │            │ │            │       │   │
│  │  │ • Entity   │ │ • Policy   │ │ • Queue    │ │ • Context  │       │   │
│  │  │ • Status   │ │ • Strategy │ │ • Scheduler│ │ • Handoff  │       │   │
│  │  │ • Events   │ │ • Executor │ │ • Worker   │ │ • Notes    │       │   │
│  │  └────────────┘ └────────────┘ └────────────┘ └────────────┘       │   │
│  │                                                                      │   │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐       │   │
│  │  │   Budget   │ │   Alert    │ │   Audit    │ │   Metrics  │       │   │
│  │  │  Domain    │ │  Domain    │ │  Domain    │ │   Domain   │       │   │
│  │  │            │ │            │ │            │ │            │       │   │
│  │  │ • Tracker  │ │ • Rules    │ │ • Logger   │ │ • Collector│       │   │
│  │  │ • Limits   │ │ • Channels │ │ • Query    │ │ • Exporter │       │   │
│  │  │ • Reports  │ │ • Throttle │ │ • Retention│ │ • Dashboard│       │   │
│  │  └────────────┘ └────────────┘ └────────────┘ └────────────┘       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      INFRASTRUCTURE LAYER                            │   │
│  │                                                                      │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │   │
│  │  │  SQLite  │ │  Event   │ │  Process │ │  Claude  │ │   Hook   │  │   │
│  │  │  Store   │ │   Bus    │ │  Scanner │ │  Parser  │ │  Server  │  │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘  │   │
│  │                                                                      │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │   │
│  │  │ Terminal │ │  Logger  │ │  Config  │ │   HTTP   │ │WebSocket │  │   │
│  │  │ Adapter  │ │          │ │  Manager │ │  Client  │ │  Server  │  │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. 文件结构设计

```
keepline/
├── src/
│   │
│   ├── index.ts                      # CLI 入口 (Commander.js)
│   │
│   ├── @types/                       # 全局类型定义
│   │   ├── index.d.ts
│   │   └── env.d.ts
│   │
│   │
│   │  ╔═══════════════════════════════════════════════════════════════╗
│   │  ║                    INTERFACE LAYER                            ║
│   │  ╚═══════════════════════════════════════════════════════════════╝
│   │
│   ├── cli/                          # CLI 命令层
│   │   ├── index.ts                  # 命令注册
│   │   ├── commands/
│   │   │   ├── list.tsx              # 列出会话
│   │   │   ├── watch.tsx             # 实时监控
│   │   │   ├── recover.ts            # 恢复会话
│   │   │   ├── daemon.ts             # 守护进程
│   │   │   ├── schedule.ts           # [NEW] 任务调度
│   │   │   ├── queue.ts              # [NEW] 任务队列
│   │   │   ├── memory.ts             # [NEW] 外部记忆
│   │   │   ├── budget.ts             # [NEW] 预算管理
│   │   │   ├── workflow.ts           # [NEW] 工作流
│   │   │   ├── status.ts             # 系统状态
│   │   │   └── web.ts                # Web 服务
│   │   └── ui/                       # Ink TUI 组件
│   │       ├── App.tsx
│   │       ├── components/
│   │       │   ├── Header.tsx
│   │       │   ├── SessionTable.tsx
│   │       │   ├── StatsBar.tsx
│   │       │   ├── BudgetWidget.tsx  # [NEW]
│   │       │   └── QueueStatus.tsx   # [NEW]
│   │       └── views/
│   │           ├── CyberpunkView.tsx
│   │           ├── DashboardView.tsx
│   │           └── MinimalView.tsx
│   │
│   ├── web/                          # Web 界面层
│   │   ├── api/
│   │   │   ├── server.ts             # Hono HTTP 服务器
│   │   │   ├── websocket.ts          # WebSocket 管理
│   │   │   ├── middleware/
│   │   │   │   ├── auth.ts           # [NEW] API 认证
│   │   │   │   ├── validation.ts
│   │   │   │   └── rateLimit.ts
│   │   │   └── routes/
│   │   │       ├── sessions.ts
│   │   │       ├── recovery.ts
│   │   │       ├── tasks.ts          # [NEW] 任务队列 API
│   │   │       ├── schedules.ts      # [NEW] 调度 API
│   │   │       ├── memory.ts         # [NEW] 记忆 API
│   │   │       ├── budget.ts         # [NEW] 预算 API
│   │   │       ├── metrics.ts        # [NEW] 指标 API
│   │   │       └── webhooks.ts       # [NEW] Webhook API
│   │   │
│   │   └── client/                   # React SPA
│   │       ├── src/
│   │       │   ├── App.tsx
│   │       │   ├── pages/
│   │       │   │   ├── Dashboard.tsx
│   │       │   │   ├── Sessions.tsx
│   │       │   │   ├── Tasks.tsx     # [NEW]
│   │       │   │   ├── Schedules.tsx # [NEW]
│   │       │   │   ├── Budget.tsx    # [NEW]
│   │       │   │   └── Settings.tsx
│   │       │   └── components/
│   │       └── vite.config.ts
│   │
│   ├── integrations/                 # [NEW] 第三方集成
│   │   ├── slack/
│   │   │   ├── bot.ts
│   │   │   ├── commands.ts
│   │   │   └── handlers.ts
│   │   ├── discord/
│   │   │   └── bot.ts
│   │   └── webhook/
│   │       ├── sender.ts
│   │       └── templates.ts
│   │
│   │
│   │  ╔═══════════════════════════════════════════════════════════════╗
│   │  ║                   APPLICATION LAYER                           ║
│   │  ╚═══════════════════════════════════════════════════════════════╝
│   │
│   ├── application/                  # [NEW] 应用服务层
│   │   ├── use-cases/                # 用例 (Use Cases)
│   │   │   ├── list-sessions.ts
│   │   │   ├── recover-session.ts
│   │   │   ├── schedule-task.ts
│   │   │   ├── execute-workflow.ts
│   │   │   └── generate-report.ts
│   │   │
│   │   ├── workflows/                # [NEW] 预定义工作流
│   │   │   ├── types.ts
│   │   │   ├── registry.ts
│   │   │   ├── executor.ts
│   │   │   └── templates/
│   │   │       ├── daily-review.yaml
│   │   │       ├── auto-fix.yaml
│   │   │       └── migration.yaml
│   │   │
│   │   └── plugins/                  # [NEW] 插件系统
│   │       ├── types.ts
│   │       ├── registry.ts
│   │       ├── loader.ts
│   │       └── hooks.ts
│   │
│   │
│   │  ╔═══════════════════════════════════════════════════════════════╗
│   │  ║                      DOMAIN LAYER                             ║
│   │  ╚═══════════════════════════════════════════════════════════════╝
│   │
│   ├── domain/                       # [RESTRUCTURED] 领域层
│   │   │
│   │   ├── session/                  # 会话领域
│   │   │   ├── entity.ts             # Session 实体
│   │   │   ├── value-objects.ts      # SessionId, Status 等
│   │   │   ├── events.ts             # SessionCreated, SessionLost 等
│   │   │   ├── service.ts            # 领域服务
│   │   │   ├── aggregator.ts         # 聚合器
│   │   │   └── repository.ts         # 仓储接口
│   │   │
│   │   ├── recovery/                 # [ENHANCED] 恢复领域
│   │   │   ├── entity.ts             # RecoveryAttempt 实体
│   │   │   ├── policy.ts             # [NEW] 恢复策略配置
│   │   │   ├── strategy.ts           # [NEW] 恢复策略 (resume/continue/new)
│   │   │   ├── evaluator.ts          # [NEW] 恢复前评估
│   │   │   ├── executor.ts           # 恢复执行器
│   │   │   ├── verifier.ts           # [NEW] 恢复后验证
│   │   │   ├── events.ts             # RecoveryStarted, RecoveryCompleted
│   │   │   └── service.ts
│   │   │
│   │   ├── task/                     # [NEW] 任务领域
│   │   │   ├── entity.ts             # Task 实体
│   │   │   ├── queue.ts              # 任务队列
│   │   │   ├── scheduler.ts          # Cron 调度器
│   │   │   ├── worker.ts             # 任务执行器
│   │   │   ├── events.ts             # TaskQueued, TaskCompleted
│   │   │   └── service.ts
│   │   │
│   │   ├── memory/                   # [NEW] 记忆领域 (借鉴 Continuous Claude)
│   │   │   ├── entity.ts             # Memory 实体
│   │   │   ├── context-builder.ts    # 上下文构建器
│   │   │   ├── handoff.ts            # 交接笔记
│   │   │   ├── extractor.ts          # 从 Hook 事件提取信息
│   │   │   └── service.ts
│   │   │
│   │   ├── budget/                   # [NEW] 预算领域 (借鉴 Sleepless Agent)
│   │   │   ├── entity.ts             # Budget 实体
│   │   │   ├── tracker.ts            # 使用量追踪
│   │   │   ├── limits.ts             # 限制配置
│   │   │   ├── scheduler-aware.ts    # 预算感知调度
│   │   │   ├── events.ts             # BudgetWarning, BudgetExceeded
│   │   │   └── service.ts
│   │   │
│   │   ├── alert/                    # [NEW] 告警领域
│   │   │   ├── entity.ts             # Alert 实体
│   │   │   ├── rule.ts               # 告警规则
│   │   │   ├── channel.ts            # 告警渠道 (Slack/Discord/Webhook)
│   │   │   ├── throttle.ts           # 防刷屏
│   │   │   ├── events.ts
│   │   │   └── service.ts
│   │   │
│   │   ├── audit/                    # [NEW] 审计领域
│   │   │   ├── entity.ts             # AuditLog 实体
│   │   │   ├── logger.ts             # 审计日志记录
│   │   │   ├── query.ts              # 查询接口
│   │   │   └── retention.ts          # 保留策略
│   │   │
│   │   └── metrics/                  # [NEW] 指标领域
│   │       ├── entity.ts             # Metric 实体
│   │       ├── collector.ts          # 指标收集
│   │       ├── exporter.ts           # Prometheus 导出
│   │       └── service.ts
│   │
│   │
│   │  ╔═══════════════════════════════════════════════════════════════╗
│   │  ║                  INFRASTRUCTURE LAYER                         ║
│   │  ╚═══════════════════════════════════════════════════════════════╝
│   │
│   ├── infrastructure/               # [RESTRUCTURED] 基础设施层
│   │   │
│   │   ├── database/                 # 数据库
│   │   │   ├── sqlite.ts             # SQLite 连接管理
│   │   │   ├── migrations/           # [NEW] 版本化迁移
│   │   │   │   ├── index.ts
│   │   │   │   ├── 001_initial.ts
│   │   │   │   ├── 002_tasks.ts
│   │   │   │   ├── 003_memory.ts
│   │   │   │   ├── 004_budget.ts
│   │   │   │   └── 005_audit.ts
│   │   │   └── repositories/         # 仓储实现
│   │   │       ├── session.repository.ts
│   │   │       ├── task.repository.ts
│   │   │       ├── memory.repository.ts
│   │   │       ├── budget.repository.ts
│   │   │       └── audit.repository.ts
│   │   │
│   │   ├── events/                   # [NEW] 事件系统
│   │   │   ├── bus.ts                # 事件总线
│   │   │   ├── store.ts              # 事件存储 (SQLite)
│   │   │   ├── handlers/             # 事件处理器
│   │   │   │   ├── session.handlers.ts
│   │   │   │   ├── recovery.handlers.ts
│   │   │   │   ├── task.handlers.ts
│   │   │   │   └── alert.handlers.ts
│   │   │   └── types.ts
│   │   │
│   │   ├── adapters/                 # 外部系统适配器
│   │   │   ├── process/
│   │   │   │   ├── scanner.ts        # 进程扫描
│   │   │   │   ├── detector.ts       # 状态检测
│   │   │   │   └── types.ts
│   │   │   ├── claude/
│   │   │   │   ├── scanner.ts        # 会话文件扫描
│   │   │   │   ├── parser/
│   │   │   │   │   ├── jsonl.ts      # JSONL 解析
│   │   │   │   │   └── history.ts
│   │   │   │   └── types.ts
│   │   │   ├── hook/
│   │   │   │   ├── server.ts         # Hook HTTP 服务器
│   │   │   │   ├── installer.ts      # Hook 安装
│   │   │   │   ├── validator.ts      # [NEW] 事件验证
│   │   │   │   ├── queue.ts          # [NEW] 事件队列 (可靠性)
│   │   │   │   └── types.ts
│   │   │   └── terminal/
│   │   │       ├── opener.ts         # 打开终端
│   │   │       ├── detector.ts       # 终端类型检测
│   │   │       └── types.ts
│   │   │
│   │   ├── daemon/                   # 守护进程
│   │   │   ├── manager.ts            # 生命周期管理
│   │   │   ├── scheduler.ts          # 定时任务
│   │   │   ├── worker.ts             # [NEW] 任务工作进程
│   │   │   ├── health.ts             # [NEW] 健康检查
│   │   │   └── ipc.ts                # [NEW] 进程间通信
│   │   │
│   │   └── shared/                   # 共享基础设施
│   │       ├── config.ts             # 配置管理
│   │       ├── logger.ts             # 日志
│   │       ├── paths.ts              # 路径工具
│   │       ├── errors.ts             # 错误类型
│   │       ├── http-client.ts        # [NEW] HTTP 客户端
│   │       └── cache.ts              # [NEW] 缓存管理
│   │
│   │
│   │  ╔═══════════════════════════════════════════════════════════════╗
│   │  ║                     SHARED KERNEL                             ║
│   │  ╚═══════════════════════════════════════════════════════════════╝
│   │
│   └── shared/                       # 共享内核
│       ├── types/                    # 通用类型
│       │   ├── result.ts             # Result<T, E> 模式
│       │   ├── option.ts             # Option<T> 模式
│       │   └── common.ts
│       ├── utils/                    # 工具函数
│       │   ├── format.ts
│       │   ├── date.ts
│       │   └── validation.ts
│       └── constants/                # 常量
│           ├── events.ts
│           └── status.ts
│
├── plugins/                          # [NEW] 插件目录
│   ├── slack-notifier/
│   │   ├── index.ts
│   │   └── package.json
│   └── prometheus-exporter/
│       ├── index.ts
│       └── package.json
│
├── workflows/                        # [NEW] 用户自定义工作流
│   └── .gitkeep
│
├── docs/
│   ├── architecture/
│   │   └── KEEPLINE_V2_ARCHITECTURE.md  # 本文件
│   ├── api/
│   └── plugins/
│
├── tests/
│   ├── unit/
│   ├── integration/
│   └── e2e/
│
├── scripts/
│   ├── build.ts
│   └── release.ts
│
├── package.json
├── tsconfig.json
├── CLAUDE.md
└── README.md
```

---

## 3. 核心领域设计

### 3.1 会话领域 (Session Domain)

```typescript
// domain/session/entity.ts
export interface Session {
  id: SessionId;
  claudeSessionId: string;
  directory: string;
  status: SessionStatus;

  // 基本信息
  title: string;
  initialPrompt: string;

  // 活动信息
  lastTool: string | null;
  lastToolInput: string | null;
  currentFile: string | null;
  lastMessage: string | null;

  // 进程信息
  pid: number | null;
  tty: string | null;

  // 统计
  toolCount: number;
  messageCount: number;

  // 时间戳
  startedAt: Date;
  lastActiveAt: Date;
  completedAt: Date | null;
}

// domain/session/value-objects.ts
export class SessionId {
  constructor(private readonly value: string) {
    if (!value || value.length < 8) {
      throw new InvalidSessionIdError(value);
    }
  }

  toString(): string { return this.value; }
  equals(other: SessionId): boolean { return this.value === other.value; }
}

export enum SessionStatus {
  Running = 'running',
  Waiting = 'waiting',
  Idle = 'idle',
  Lost = 'lost',
  Completed = 'completed'
}

// domain/session/events.ts
export interface SessionDiscovered extends DomainEvent {
  type: 'session.discovered';
  sessionId: SessionId;
  directory: string;
  timestamp: Date;
}

export interface SessionLost extends DomainEvent {
  type: 'session.lost';
  sessionId: SessionId;
  lastActiveAt: Date;
  reason: 'process_terminated' | 'timeout' | 'unknown';
}

export interface SessionRecovered extends DomainEvent {
  type: 'session.recovered';
  sessionId: SessionId;
  method: RecoveryMethod;
  newPid: number;
}
```

### 3.2 恢复领域 (Recovery Domain)

```typescript
// domain/recovery/policy.ts
export interface RecoveryPolicy {
  id: string;
  name: string;
  enabled: boolean;

  // 触发条件
  trigger: {
    status: 'lost';
    minAgeSeconds: number;    // 丢失多久后触发 (e.g., 60)
    maxAgeSeconds: number;    // 超过多久放弃 (e.g., 3600)
    directoryPattern?: string; // 可选: 目录匹配
    priority?: number;         // 优先级
  };

  // 执行策略
  action: {
    method: 'resume' | 'continue' | 'new' | 'evaluate';
    injectContext: boolean;
    openTerminal: boolean;
    terminalApp?: 'Terminal' | 'iTerm' | 'Warp' | 'auto';
    skipPermissions: boolean;
  };

  // 重试配置
  retry: {
    maxAttempts: number;
    backoff: 'linear' | 'exponential';
    initialDelayMs: number;
    maxDelayMs: number;
  };

  // 通知配置
  notify: {
    onStart: boolean;
    onSuccess: boolean;
    onFailure: boolean;
    channels: string[];
  };
}

// domain/recovery/evaluator.ts
export interface RecoveryAssessment {
  sessionId: SessionId;
  isRecoverable: boolean;
  reason: string;
  riskLevel: 'low' | 'medium' | 'high';

  // 推荐
  recommendedMethod: RecoveryMethod;
  contextAvailable: boolean;
  estimatedSuccessRate: number;

  // 检查项
  checks: {
    sessionFileExists: boolean;
    directoryExists: boolean;
    lastActivityAge: number;
    previousAttempts: number;
  };
}

// domain/recovery/strategy.ts
export interface RecoveryStrategy {
  name: string;
  canHandle(session: Session, assessment: RecoveryAssessment): boolean;
  execute(session: Session, options: RecoveryOptions): Promise<RecoveryResult>;
}

export class ResumeStrategy implements RecoveryStrategy {
  name = 'resume';

  canHandle(session: Session, assessment: RecoveryAssessment): boolean {
    return assessment.checks.sessionFileExists;
  }

  async execute(session: Session, options: RecoveryOptions): Promise<RecoveryResult> {
    const command = `claude --resume ${session.claudeSessionId}`;
    // ... 执行恢复
  }
}
```

### 3.3 任务领域 (Task Domain)

```typescript
// domain/task/entity.ts
export interface Task {
  id: TaskId;
  type: TaskType;

  // 任务定义
  name: string;
  prompt?: string;
  sessionId?: SessionId;
  directory: string;

  // 调度信息
  schedule?: {
    cron: string;           // "0 8 * * *"
    timezone: string;       // "Asia/Shanghai"
    nextRunAt: Date;
  };

  // 状态
  status: TaskStatus;
  priority: number;

  // 执行配置
  timeout: number;
  retryCount: number;
  maxRetries: number;

  // 依赖
  dependencies: TaskId[];

  // 元数据
  createdAt: Date;
  startedAt?: Date;
  completedAt?: Date;
  error?: string;
  cost?: number;
}

export enum TaskType {
  Recovery = 'recovery',      // 恢复会话
  Scheduled = 'scheduled',    // 定时任务
  Manual = 'manual',          // 手动触发
  Workflow = 'workflow',      // 工作流步骤
  AutoGenerated = 'auto'      // 自动生成
}

export enum TaskStatus {
  Pending = 'pending',
  Queued = 'queued',
  Running = 'running',
  Completed = 'completed',
  Failed = 'failed',
  Cancelled = 'cancelled'
}

// domain/task/queue.ts
export interface TaskQueue {
  enqueue(task: Task): Promise<void>;
  dequeue(): Promise<Task | null>;
  peek(): Promise<Task | null>;
  size(): Promise<number>;

  // 优先级操作
  prioritize(taskId: TaskId, priority: number): Promise<void>;

  // 状态操作
  cancel(taskId: TaskId): Promise<void>;
  retry(taskId: TaskId): Promise<void>;
}

// domain/task/scheduler.ts
export interface TaskScheduler {
  schedule(task: Task): Promise<void>;
  unschedule(taskId: TaskId): Promise<void>;
  getNextRun(taskId: TaskId): Promise<Date | null>;

  // 调度控制
  pause(): void;
  resume(): void;

  // 查询
  getScheduledTasks(): Promise<Task[]>;
}
```

### 3.4 记忆领域 (Memory Domain)

```typescript
// domain/memory/entity.ts
export interface SessionMemory {
  sessionId: SessionId;
  directory: string;

  // 进度追踪
  lastProgress: string;
  pendingTasks: string[];
  completedTasks: string[];

  // 上下文
  knownIssues: string[];
  decisions: string[];
  notes: string;

  // 交接信息
  handoffNotes: string;       // 给下一次迭代的笔记
  handoffPriority: string[];  // 优先处理事项

  // 元数据
  iterationCount: number;
  totalTokensUsed: number;
  updatedAt: Date;
}

// domain/memory/context-builder.ts
export class ContextBuilder {
  build(memory: SessionMemory): string {
    return `
## 上次会话上下文

**最后进展**: ${memory.lastProgress}

**待完成任务**:
${memory.pendingTasks.map(t => `- [ ] ${t}`).join('\n')}

**已完成任务**:
${memory.completedTasks.slice(-5).map(t => `- [x] ${t}`).join('\n')}

**已知问题**:
${memory.knownIssues.map(i => `- ${i}`).join('\n')}

**重要决策**:
${memory.decisions.map(d => `- ${d}`).join('\n')}

**交接笔记**:
${memory.handoffNotes}

---
这是第 ${memory.iterationCount + 1} 次迭代。请继续从上次的进度开始工作。
完成后，请更新进度并留下笔记给下一次迭代。
    `.trim();
  }
}

// domain/memory/extractor.ts
export class MemoryExtractor {
  // 从 Claude 输出中提取进度信息
  extractFromOutput(output: string): Partial<SessionMemory> {
    // 使用正则或简单解析提取:
    // - 完成的任务 (包含 ✓ 或 completed)
    // - 待办事项 (包含 TODO 或 [ ])
    // - 问题 (包含 issue 或 problem)
    // - 决策 (包含 decided 或 chose)
  }

  // 从 Hook 事件中提取信息
  extractFromHookEvent(event: HookEvent): Partial<SessionMemory> {
    return {
      lastProgress: `使用 ${event.toolName} 处理 ${event.toolInput?.substring(0, 100)}`,
      // ...
    };
  }
}
```

### 3.5 预算领域 (Budget Domain)

```typescript
// domain/budget/entity.ts
export interface Budget {
  id: string;
  name: string;

  // 限制
  limits: {
    daily: number;          // $10
    monthly: number;        // $300
    perSession: number;     // $5
  };

  // 当前使用
  usage: {
    today: number;
    thisMonth: number;
    total: number;
  };

  // 告警阈值
  alerts: {
    warningThreshold: number;   // 0.8 (80%)
    pauseThreshold: number;     // 0.95 (95%)
  };

  // 智能调度
  scheduling: {
    nighttimeRatio: number;     // 夜间可使用比例
    workHoursStart: number;     // 9
    workHoursEnd: number;       // 18
    timezone: string;
  };
}

// domain/budget/tracker.ts
export class BudgetTracker {
  async record(sessionId: SessionId, usage: UsageRecord): Promise<void> {
    // 记录使用量
    await this.repository.addUsage(sessionId, usage);

    // 检查是否超限
    const budget = await this.getCurrentBudget();
    const ratio = budget.usage.today / budget.limits.daily;

    if (ratio > budget.alerts.pauseThreshold) {
      this.eventBus.publish(new BudgetExceeded(budget));
    } else if (ratio > budget.alerts.warningThreshold) {
      this.eventBus.publish(new BudgetWarning(budget, ratio));
    }
  }

  async shouldAllowTask(task: Task): Promise<boolean> {
    const budget = await this.getCurrentBudget();
    const isNighttime = this.isNighttime(budget.scheduling);

    const threshold = isNighttime
      ? budget.alerts.pauseThreshold
      : budget.alerts.warningThreshold;

    return (budget.usage.today / budget.limits.daily) < threshold;
  }
}

// domain/budget/events.ts
export interface BudgetWarning extends DomainEvent {
  type: 'budget.warning';
  budget: Budget;
  usageRatio: number;
  message: string;
}

export interface BudgetExceeded extends DomainEvent {
  type: 'budget.exceeded';
  budget: Budget;
  action: 'pause_new_tasks' | 'notify_only';
}
```

---

## 4. 事件驱动架构

### 4.1 事件总线

```typescript
// infrastructure/events/bus.ts
export interface DomainEvent {
  type: string;
  timestamp: Date;
  aggregateId?: string;
  payload?: Record<string, unknown>;
}

export interface EventHandler<T extends DomainEvent = DomainEvent> {
  handle(event: T): Promise<void>;
}

export class EventBus {
  private handlers = new Map<string, EventHandler[]>();
  private eventStore: EventStore;

  constructor(eventStore: EventStore) {
    this.eventStore = eventStore;
  }

  subscribe<T extends DomainEvent>(
    eventType: string,
    handler: EventHandler<T>
  ): void {
    const handlers = this.handlers.get(eventType) || [];
    handlers.push(handler as EventHandler);
    this.handlers.set(eventType, handlers);
  }

  async publish<T extends DomainEvent>(event: T): Promise<void> {
    // 1. 持久化事件
    await this.eventStore.append(event);

    // 2. 通知处理器 (失败不影响其他处理器)
    const handlers = this.handlers.get(event.type) || [];

    await Promise.allSettled(
      handlers.map(handler =>
        handler.handle(event).catch(err => {
          logger.error(`Event handler failed: ${event.type}`, err);
        })
      )
    );
  }

  // 重放事件 (用于重建状态)
  async replay(fromTimestamp: Date): Promise<void> {
    const events = await this.eventStore.getEvents(fromTimestamp);
    for (const event of events) {
      await this.publish(event);
    }
  }
}
```

### 4.2 事件存储

```typescript
// infrastructure/events/store.ts
export class EventStore {
  constructor(private db: Database) {}

  async append(event: DomainEvent): Promise<void> {
    await this.db.run(`
      INSERT INTO events (type, aggregate_id, payload, timestamp)
      VALUES (?, ?, ?, ?)
    `, [
      event.type,
      event.aggregateId || null,
      JSON.stringify(event.payload),
      event.timestamp.toISOString()
    ]);
  }

  async getEvents(
    fromTimestamp?: Date,
    aggregateId?: string
  ): Promise<DomainEvent[]> {
    let query = 'SELECT * FROM events WHERE 1=1';
    const params: any[] = [];

    if (fromTimestamp) {
      query += ' AND timestamp >= ?';
      params.push(fromTimestamp.toISOString());
    }

    if (aggregateId) {
      query += ' AND aggregate_id = ?';
      params.push(aggregateId);
    }

    query += ' ORDER BY timestamp ASC';

    const rows = await this.db.all(query, params);
    return rows.map(row => ({
      type: row.type,
      aggregateId: row.aggregate_id,
      payload: JSON.parse(row.payload),
      timestamp: new Date(row.timestamp)
    }));
  }
}
```

### 4.3 事件处理器示例

```typescript
// infrastructure/events/handlers/session.handlers.ts
export class SessionLostHandler implements EventHandler<SessionLost> {
  constructor(
    private recoveryService: RecoveryService,
    private alertService: AlertService,
    private memoryService: MemoryService
  ) {}

  async handle(event: SessionLost): Promise<void> {
    // 1. 保存当前上下文到记忆
    await this.memoryService.saveContext(event.sessionId);

    // 2. 检查是否有自动恢复策略
    const policy = await this.recoveryService.findMatchingPolicy(event.sessionId);

    if (policy && policy.enabled) {
      // 3. 排队自动恢复任务
      await this.recoveryService.scheduleRecovery(event.sessionId, policy);
    }

    // 4. 发送告警
    await this.alertService.send({
      type: 'session_lost',
      sessionId: event.sessionId.toString(),
      lastActiveAt: event.lastActiveAt,
      autoRecoveryScheduled: !!policy
    });
  }
}

// infrastructure/events/handlers/recovery.handlers.ts
export class RecoveryCompletedHandler implements EventHandler<RecoveryCompleted> {
  constructor(
    private memoryService: MemoryService,
    private alertService: AlertService
  ) {}

  async handle(event: RecoveryCompleted): Promise<void> {
    if (event.success) {
      // 注入上下文到恢复的会话
      const context = await this.memoryService.getRecoveryContext(event.sessionId);
      // ... 注入逻辑

      await this.alertService.send({
        type: 'recovery_success',
        sessionId: event.sessionId.toString(),
        method: event.method
      });
    } else {
      await this.alertService.send({
        type: 'recovery_failed',
        sessionId: event.sessionId.toString(),
        error: event.error,
        attemptsRemaining: event.attemptsRemaining
      });
    }
  }
}
```

---

## 5. 数据库设计

### 5.1 完整 Schema

```sql
-- ============================================================
-- 核心表
-- ============================================================

-- 会话表 (已有，扩展)
CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  session_id TEXT UNIQUE NOT NULL,
  directory TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'idle',
  title TEXT,
  initial_prompt TEXT NOT NULL,
  last_tool TEXT,
  last_tool_input TEXT,
  current_file TEXT,
  last_message TEXT,
  started_at TEXT,
  last_active_at TEXT NOT NULL,
  completed_at TEXT,
  pid INTEGER,
  tty TEXT,
  tool_count INTEGER DEFAULT 0,
  message_count INTEGER DEFAULT 0,
  -- [NEW] 关联记忆
  memory_id TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_sessions_status ON sessions(status);
CREATE INDEX idx_sessions_directory ON sessions(directory);
CREATE INDEX idx_sessions_last_active ON sessions(last_active_at);

-- ============================================================
-- 任务系统表 [NEW]
-- ============================================================

-- 任务表
CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  type TEXT NOT NULL,              -- recovery, scheduled, manual, workflow
  name TEXT NOT NULL,
  prompt TEXT,
  session_id TEXT,
  directory TEXT NOT NULL,

  -- 调度
  cron TEXT,
  timezone TEXT DEFAULT 'UTC',
  next_run_at TEXT,

  -- 状态
  status TEXT NOT NULL DEFAULT 'pending',
  priority INTEGER DEFAULT 0,

  -- 执行
  timeout INTEGER DEFAULT 300000,  -- 5 minutes
  retry_count INTEGER DEFAULT 0,
  max_retries INTEGER DEFAULT 3,

  -- 结果
  started_at TEXT,
  completed_at TEXT,
  error TEXT,
  cost REAL,

  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

  FOREIGN KEY (session_id) REFERENCES sessions(session_id)
);

CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_next_run ON tasks(next_run_at);
CREATE INDEX idx_tasks_priority ON tasks(priority DESC);

-- 任务依赖表
CREATE TABLE task_dependencies (
  task_id TEXT NOT NULL,
  depends_on_task_id TEXT NOT NULL,
  PRIMARY KEY (task_id, depends_on_task_id),
  FOREIGN KEY (task_id) REFERENCES tasks(id),
  FOREIGN KEY (depends_on_task_id) REFERENCES tasks(id)
);

-- ============================================================
-- 记忆系统表 [NEW]
-- ============================================================

-- 会话记忆表
CREATE TABLE session_memories (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  directory TEXT NOT NULL,

  -- 进度
  last_progress TEXT,
  pending_tasks TEXT,              -- JSON array
  completed_tasks TEXT,            -- JSON array

  -- 上下文
  known_issues TEXT,               -- JSON array
  decisions TEXT,                  -- JSON array
  notes TEXT,

  -- 交接
  handoff_notes TEXT,
  handoff_priority TEXT,           -- JSON array

  -- 元数据
  iteration_count INTEGER DEFAULT 0,
  total_tokens_used INTEGER DEFAULT 0,

  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

  FOREIGN KEY (session_id) REFERENCES sessions(session_id)
);

CREATE INDEX idx_memories_session ON session_memories(session_id);
CREATE INDEX idx_memories_directory ON session_memories(directory);

-- ============================================================
-- 恢复系统表 [NEW]
-- ============================================================

-- 恢复策略表
CREATE TABLE recovery_policies (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  enabled INTEGER DEFAULT 1,

  -- 触发条件 (JSON)
  trigger_config TEXT NOT NULL,

  -- 执行策略 (JSON)
  action_config TEXT NOT NULL,

  -- 重试配置 (JSON)
  retry_config TEXT NOT NULL,

  -- 通知配置 (JSON)
  notify_config TEXT,

  priority INTEGER DEFAULT 0,

  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 恢复尝试记录表
CREATE TABLE recovery_attempts (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  policy_id TEXT,

  method TEXT NOT NULL,            -- resume, continue, new
  status TEXT NOT NULL,            -- pending, running, success, failed

  started_at TEXT NOT NULL,
  completed_at TEXT,

  error TEXT,
  new_pid INTEGER,

  -- 评估结果 (JSON)
  assessment TEXT,

  FOREIGN KEY (session_id) REFERENCES sessions(session_id),
  FOREIGN KEY (policy_id) REFERENCES recovery_policies(id)
);

CREATE INDEX idx_recovery_session ON recovery_attempts(session_id);
CREATE INDEX idx_recovery_status ON recovery_attempts(status);

-- ============================================================
-- 预算系统表 [NEW]
-- ============================================================

-- 预算配置表
CREATE TABLE budgets (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL DEFAULT 'default',

  -- 限制 (JSON)
  limits_config TEXT NOT NULL,

  -- 告警阈值 (JSON)
  alerts_config TEXT NOT NULL,

  -- 智能调度 (JSON)
  scheduling_config TEXT,

  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 使用量记录表
CREATE TABLE usage_records (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT,

  input_tokens INTEGER DEFAULT 0,
  output_tokens INTEGER DEFAULT 0,
  cache_read_tokens INTEGER DEFAULT 0,
  cache_write_tokens INTEGER DEFAULT 0,

  cost REAL DEFAULT 0,
  model TEXT,

  recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

  FOREIGN KEY (session_id) REFERENCES sessions(session_id)
);

CREATE INDEX idx_usage_session ON usage_records(session_id);
CREATE INDEX idx_usage_recorded ON usage_records(recorded_at);

-- 每日汇总表 (用于快速查询)
CREATE TABLE daily_usage (
  date TEXT PRIMARY KEY,           -- YYYY-MM-DD
  total_cost REAL DEFAULT 0,
  total_input_tokens INTEGER DEFAULT 0,
  total_output_tokens INTEGER DEFAULT 0,
  session_count INTEGER DEFAULT 0,
  task_count INTEGER DEFAULT 0
);

-- ============================================================
-- 告警系统表 [NEW]
-- ============================================================

-- 告警规则表
CREATE TABLE alert_rules (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  enabled INTEGER DEFAULT 1,

  event_type TEXT NOT NULL,        -- session_lost, budget_warning, etc.
  condition TEXT,                  -- 条件表达式

  severity TEXT DEFAULT 'info',    -- info, warning, critical
  channels TEXT NOT NULL,          -- JSON array of channel ids

  template TEXT,                   -- 消息模板
  cooldown_seconds INTEGER DEFAULT 300,

  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 告警渠道表
CREATE TABLE alert_channels (
  id TEXT PRIMARY KEY,
  type TEXT NOT NULL,              -- slack, discord, webhook, email
  name TEXT NOT NULL,

  config TEXT NOT NULL,            -- JSON (webhook url, token, etc.)
  enabled INTEGER DEFAULT 1,

  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 告警历史表
CREATE TABLE alert_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  rule_id TEXT,
  channel_id TEXT,

  event_type TEXT NOT NULL,
  severity TEXT NOT NULL,
  message TEXT NOT NULL,

  sent_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  status TEXT DEFAULT 'sent',      -- sent, failed, throttled
  error TEXT,

  FOREIGN KEY (rule_id) REFERENCES alert_rules(id),
  FOREIGN KEY (channel_id) REFERENCES alert_channels(id)
);

CREATE INDEX idx_alerts_sent ON alert_history(sent_at);

-- ============================================================
-- 审计系统表 [NEW]
-- ============================================================

CREATE TABLE audit_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,

  actor TEXT NOT NULL,             -- system, user, api, plugin
  action TEXT NOT NULL,            -- recover, stop, schedule, etc.

  resource_type TEXT,              -- session, task, policy
  resource_id TEXT,

  details TEXT,                    -- JSON
  ip_address TEXT,

  timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_audit_timestamp ON audit_logs(timestamp);
CREATE INDEX idx_audit_resource ON audit_logs(resource_type, resource_id);

-- ============================================================
-- 事件存储表 [NEW]
-- ============================================================

CREATE TABLE events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  type TEXT NOT NULL,
  aggregate_id TEXT,
  payload TEXT,                    -- JSON
  timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_events_type ON events(type);
CREATE INDEX idx_events_aggregate ON events(aggregate_id);
CREATE INDEX idx_events_timestamp ON events(timestamp);

-- ============================================================
-- Hook 事件表 (已有，保留)
-- ============================================================

CREATE TABLE hook_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT,
  event_type TEXT NOT NULL,
  payload TEXT,
  timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================
-- 元数据表 (已有，保留)
-- ============================================================

CREATE TABLE metadata (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================
-- 工作流表 [NEW]
-- ============================================================

CREATE TABLE workflows (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,

  -- 步骤定义 (JSON)
  steps TEXT NOT NULL,

  -- 配置
  enabled INTEGER DEFAULT 1,
  timeout INTEGER,

  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE workflow_runs (
  id TEXT PRIMARY KEY,
  workflow_id TEXT NOT NULL,

  status TEXT NOT NULL,            -- pending, running, completed, failed
  current_step INTEGER DEFAULT 0,

  started_at TEXT,
  completed_at TEXT,

  -- 每步结果 (JSON)
  step_results TEXT,

  error TEXT,

  FOREIGN KEY (workflow_id) REFERENCES workflows(id)
);
```

---

## 6. API 设计

### 6.1 REST API 端点

```yaml
# ============================================================
# 会话管理 (已有，扩展)
# ============================================================

GET /api/sessions
  query:
    - limit: number (default: 50, max: 100)
    - offset: number
    - status: string (running|waiting|idle|lost|completed)
    - directory: string
    - sort: string (lastActiveAt|startedAt|directory)
    - order: string (asc|desc)
  response:
    sessions: Session[]
    stats: { running, waiting, idle, lost, completed }
    pagination: { total, limit, offset }

GET /api/sessions/:id
  response: Session

GET /api/sessions/:id/memory        # [NEW]
  response: SessionMemory

POST /api/sessions/:id/recover
  body:
    method: string (resume|continue|new)
    terminal?: string
    injectContext?: boolean         # [NEW]
  response:
    command: string
    success: boolean

POST /api/sessions/:id/stop
  body:
    signal?: string (SIGTERM|SIGKILL)
  response:
    success: boolean

# ============================================================
# 任务管理 [NEW]
# ============================================================

GET /api/tasks
  query:
    - status: string
    - type: string
    - limit: number
    - offset: number
  response:
    tasks: Task[]
    stats: { pending, running, completed, failed }

POST /api/tasks
  body:
    type: string
    name: string
    prompt?: string
    directory: string
    schedule?: { cron, timezone }
    priority?: number
  response:
    task: Task

GET /api/tasks/:id
  response: Task

DELETE /api/tasks/:id
  response: { success: boolean }

POST /api/tasks/:id/cancel
  response: { success: boolean }

POST /api/tasks/:id/retry
  response: { success: boolean }

# ============================================================
# 调度管理 [NEW]
# ============================================================

GET /api/schedules
  response:
    schedules: ScheduledTask[]

POST /api/schedules
  body:
    name: string
    cron: string
    prompt: string
    directory: string
    timezone?: string
  response:
    schedule: ScheduledTask

PUT /api/schedules/:id
  body: Partial<ScheduledTask>
  response:
    schedule: ScheduledTask

DELETE /api/schedules/:id
  response: { success: boolean }

POST /api/schedules/:id/enable
POST /api/schedules/:id/disable
  response: { success: boolean }

# ============================================================
# 记忆管理 [NEW]
# ============================================================

GET /api/memory
  query:
    - directory?: string
  response:
    memories: SessionMemory[]

GET /api/memory/:sessionId
  response: SessionMemory

PUT /api/memory/:sessionId
  body:
    notes?: string
    handoffNotes?: string
    pendingTasks?: string[]
  response:
    memory: SessionMemory

# ============================================================
# 预算管理 [NEW]
# ============================================================

GET /api/budget
  response:
    budget: Budget
    usage: {
      today: number
      thisMonth: number
      total: number
    }
    forecast: {
      endOfDayEstimate: number
      endOfMonthEstimate: number
    }

GET /api/budget/history
  query:
    - from: string (ISO date)
    - to: string (ISO date)
    - granularity: string (day|week|month)
  response:
    history: UsageRecord[]

PUT /api/budget
  body:
    limits?: { daily, monthly, perSession }
    alerts?: { warningThreshold, pauseThreshold }
  response:
    budget: Budget

# ============================================================
# 恢复策略 [NEW]
# ============================================================

GET /api/recovery/policies
  response:
    policies: RecoveryPolicy[]

POST /api/recovery/policies
  body: RecoveryPolicy
  response:
    policy: RecoveryPolicy

PUT /api/recovery/policies/:id
  body: Partial<RecoveryPolicy>
  response:
    policy: RecoveryPolicy

DELETE /api/recovery/policies/:id
  response: { success: boolean }

GET /api/recovery/attempts
  query:
    - sessionId?: string
    - status?: string
    - limit: number
  response:
    attempts: RecoveryAttempt[]

# ============================================================
# 告警管理 [NEW]
# ============================================================

GET /api/alerts/rules
  response:
    rules: AlertRule[]

POST /api/alerts/rules
  body: AlertRule
  response:
    rule: AlertRule

GET /api/alerts/channels
  response:
    channels: AlertChannel[]

POST /api/alerts/channels
  body: AlertChannel
  response:
    channel: AlertChannel

POST /api/alerts/test/:channelId
  body:
    message: string
  response:
    success: boolean

GET /api/alerts/history
  query:
    - from?: string
    - to?: string
    - severity?: string
  response:
    alerts: AlertHistory[]

# ============================================================
# 指标 [NEW]
# ============================================================

GET /api/metrics
  response:
    # Prometheus 格式
    text/plain

GET /api/metrics/dashboard
  response:
    sessions: { running, waiting, idle, lost, completed }
    tasks: { pending, running, completed, failed }
    budget: { usedToday, limitToday, percentUsed }
    recoveries: { total, successful, failed }

# ============================================================
# 工作流 [NEW]
# ============================================================

GET /api/workflows
  response:
    workflows: Workflow[]

POST /api/workflows
  body: Workflow
  response:
    workflow: Workflow

POST /api/workflows/:id/run
  body:
    directory: string
    variables?: Record<string, string>
  response:
    run: WorkflowRun

GET /api/workflows/runs
  query:
    - workflowId?: string
    - status?: string
  response:
    runs: WorkflowRun[]

GET /api/workflows/runs/:id
  response:
    run: WorkflowRun

# ============================================================
# 审计 [NEW]
# ============================================================

GET /api/audit
  query:
    - from?: string
    - to?: string
    - actor?: string
    - action?: string
    - resourceType?: string
  response:
    logs: AuditLog[]
```

### 6.2 WebSocket 事件

```typescript
// 客户端 → 服务器
interface ClientMessage {
  type: 'subscribe' | 'unsubscribe';
  channels: string[];  // 'sessions', 'tasks', 'alerts', 'metrics'
}

// 服务器 → 客户端
interface ServerMessage {
  type: string;
  channel: string;
  data: unknown;
  timestamp: string;
}

// 事件类型
type ServerEventType =
  // 会话事件
  | 'session:discovered'
  | 'session:updated'
  | 'session:lost'
  | 'session:recovered'
  // 任务事件
  | 'task:queued'
  | 'task:started'
  | 'task:completed'
  | 'task:failed'
  // 告警事件
  | 'alert:triggered'
  // 预算事件
  | 'budget:warning'
  | 'budget:exceeded'
  // 系统事件
  | 'sync:complete'
  | 'daemon:health';
```

---

## 7. CLI 命令设计

```bash
# ============================================================
# 会话管理 (已有)
# ============================================================

keepline                              # 默认: 列出会话
keepline list [options]               # 列出会话
  -s, --status <status>             # 按状态过滤
  -d, --directory <dir>             # 按目录过滤
  -l, --limit <n>                   # 限制数量
  --style <style>                   # UI 样式

keepline watch [options]              # 实时监控
  -i, --interval <seconds>          # 刷新间隔

keepline recover [session-id]         # 恢复会话
  -m, --method <method>             # resume/continue/new
  -t, --terminal                    # 在新终端打开
  --with-context                    # [NEW] 注入上下文

# ============================================================
# 任务管理 [NEW]
# ============================================================

keepline queue                        # 查看任务队列
keepline queue add [options]          # 添加任务
  --prompt <prompt>                 # 任务提示
  --directory <dir>                 # 工作目录
  --priority <n>                    # 优先级 (0-10)
  --depends-on <task-id>            # 依赖任务

keepline queue status                 # 队列状态
keepline queue pause                  # 暂停队列
keepline queue resume                 # 恢复队列
keepline queue cancel <task-id>       # 取消任务
keepline queue retry <task-id>        # 重试任务

# ============================================================
# 调度管理 [NEW]
# ============================================================

keepline schedule                     # 查看调度任务
keepline schedule add [options]       # 添加调度
  --name <name>                     # 任务名称
  --cron <cron>                     # Cron 表达式
  --prompt <prompt>                 # 任务提示
  --directory <dir>                 # 工作目录
  --timezone <tz>                   # 时区

keepline schedule list                # 列出调度
keepline schedule enable <id>         # 启用调度
keepline schedule disable <id>        # 禁用调度
keepline schedule delete <id>         # 删除调度
keepline schedule run <id>            # 立即运行

# ============================================================
# 记忆管理 [NEW]
# ============================================================

keepline memory                       # 查看所有记忆
keepline memory show <session-id>     # 查看会话记忆
keepline memory edit <session-id>     # 编辑记忆 (打开编辑器)
keepline memory clear <session-id>    # 清除记忆
keepline memory export [options]      # 导出记忆
  --format <format>                 # json/markdown
  --output <file>                   # 输出文件

# ============================================================
# 预算管理 [NEW]
# ============================================================

keepline budget                       # 查看预算状态
keepline budget status                # 详细状态
keepline budget set [options]         # 设置预算
  --daily <amount>                  # 每日限额
  --monthly <amount>                # 每月限额
  --warning <percent>               # 警告阈值

keepline budget history [options]     # 使用历史
  --from <date>                     # 开始日期
  --to <date>                       # 结束日期
  --format <format>                 # table/json/csv

# ============================================================
# 恢复策略 [NEW]
# ============================================================

keepline policy                       # 查看恢复策略
keepline policy add [options]         # 添加策略
  --name <name>
  --min-age <seconds>
  --max-age <seconds>
  --method <method>
  --auto-context                    # 自动注入上下文
  --notify <channels>               # 通知渠道

keepline policy enable <id>
keepline policy disable <id>
keepline policy delete <id>

# ============================================================
# 工作流 [NEW]
# ============================================================

keepline workflow                     # 查看工作流
keepline workflow list                # 列出工作流
keepline workflow create <name>       # 创建工作流 (交互式)
keepline workflow run <name> [dir]    # 运行工作流
keepline workflow status <run-id>     # 查看运行状态

# ============================================================
# 告警 [NEW]
# ============================================================

keepline alert                        # 查看告警配置
keepline alert add-channel [options]  # 添加告警渠道
  --type <type>                     # slack/discord/webhook
  --name <name>
  --webhook <url>

keepline alert add-rule [options]     # 添加告警规则
  --event <event>                   # 事件类型
  --severity <level>
  --channels <ids>

keepline alert test <channel-id>      # 测试告警
keepline alert history                # 告警历史

# ============================================================
# 守护进程 (已有，扩展)
# ============================================================

keepline daemon start [options]
  --hooks                           # 同时安装 hooks
  --scheduler                       # [NEW] 启用调度器
  --workers <n>                     # [NEW] 工作进程数

keepline daemon stop
keepline daemon restart
keepline daemon status
keepline daemon logs                  # [NEW] 查看日志

# ============================================================
# 系统 (已有，扩展)
# ============================================================

keepline status                       # 系统状态
keepline sync                         # 手动同步
keepline web                          # 启动 Web UI

keepline metrics                      # [NEW] 查看指标
keepline audit [options]              # [NEW] 查看审计日志
  --from <date>
  --action <action>

keepline config                       # 查看配置
keepline config set <key> <value>     # 设置配置
keepline config reset                 # 重置配置
```

---

## 8. 实施路线图

### Phase 1: 基础重构 (1-2 周)

**目标**: 重构为清晰的分层架构

- [ ] 创建 `domain/` 目录结构
- [ ] 迁移现有代码到新结构
- [ ] 实现事件总线和事件存储
- [ ] 添加数据库迁移系统
- [ ] 编写基础单元测试

### Phase 2: 记忆系统 (1 周)

**目标**: 实现上下文持久化 (借鉴 Continuous Claude)

- [ ] 实现 `SessionMemory` 实体
- [ ] 实现 `MemoryExtractor` 从 Hook 事件提取信息
- [ ] 实现 `ContextBuilder` 构建恢复上下文
- [ ] 添加 `--with-context` 恢复选项
- [ ] 添加 `keepline memory` 命令

### Phase 3: 自动恢复 (1-2 周)

**目标**: 实现策略驱动的自动恢复

- [ ] 实现 `RecoveryPolicy` 配置
- [ ] 实现 `RecoveryEvaluator` 评估
- [ ] 实现三阶段恢复: 评估 → 执行 → 验证
- [ ] 实现重试逻辑
- [ ] 添加 `keepline policy` 命令
- [ ] 集成事件系统

### Phase 4: 任务队列 (1-2 周)

**目标**: 实现任务调度系统

- [ ] 实现 `TaskQueue` 持久化队列
- [ ] 实现 `TaskScheduler` Cron 调度
- [ ] 实现 `TaskWorker` 执行器
- [ ] 添加 `keepline queue` 命令
- [ ] 添加 `keepline schedule` 命令

### Phase 5: 预算管理 (1 周)

**目标**: 实现成本控制 (借鉴 Sleepless Agent)

- [ ] 实现 `BudgetTracker`
- [ ] 实现预算感知调度
- [ ] 添加使用量记录
- [ ] 添加 `keepline budget` 命令
- [ ] 集成告警系统

### Phase 6: 告警系统 (1 周)

**目标**: 实现多渠道通知

- [ ] 实现 `AlertService`
- [ ] 实现 Slack 集成
- [ ] 实现 Discord 集成
- [ ] 实现 Webhook 集成
- [ ] 添加 `keepline alert` 命令
- [ ] 实现节流/防刷屏

### Phase 7: 可观测性 (1 周)

**目标**: 实现监控和审计

- [ ] 实现 Prometheus 指标导出
- [ ] 实现审计日志
- [ ] 扩展 Web UI 仪表板
- [ ] 添加健康检查端点

### Phase 8: 工作流 & 插件 (2 周)

**目标**: 实现高级自动化

- [ ] 实现工作流定义和执行
- [ ] 实现插件系统
- [ ] 编写示例工作流
- [ ] 编写示例插件

---

## 9. 配置文件设计

```typescript
// ~/.keepline/config.json
interface KeeplineConfig {
  // 基础配置 (已有)
  scanInterval: number;
  hookPort: number;
  logLevel: 'debug' | 'info' | 'warn' | 'error';
  fileLogging: boolean;

  // [NEW] 恢复配置
  recovery: {
    enabled: boolean;
    defaultPolicy: string;  // policy id
  };

  // [NEW] 任务配置
  tasks: {
    maxConcurrency: number;
    defaultTimeout: number;
    retryBackoff: 'linear' | 'exponential';
  };

  // [NEW] 预算配置
  budget: {
    enabled: boolean;
    limits: {
      daily: number;
      monthly: number;
      perSession: number;
    };
    alerts: {
      warningThreshold: number;
      pauseThreshold: number;
    };
    scheduling: {
      nighttimeRatio: number;
      workHoursStart: number;
      workHoursEnd: number;
      timezone: string;
    };
  };

  // [NEW] 告警配置
  alerts: {
    enabled: boolean;
    defaultCooldown: number;
  };

  // [NEW] 插件配置
  plugins: {
    enabled: boolean;
    directory: string;
    autoload: boolean;
  };
}
```

---

## 10. 总结

### 核心变化

| 领域 | v1 | v2 |
|------|----|----|
| **架构** | 扁平结构 | 分层 DDD |
| **恢复** | 手动触发 | 策略驱动自动恢复 |
| **上下文** | 无持久化 | 外部记忆系统 |
| **调度** | 固定间隔 | Cron + 任务队列 |
| **成本** | 无控制 | 预算管理 + 智能调度 |
| **通知** | 无 | 多渠道告警 |
| **可观测** | 日志 | 指标 + 审计 |

### 设计原则

1. **接力赛模式**: 每次迭代只做一件事，留笔记给下次
2. **容错优先**: 失败是正常的，自动重试和降级
3. **人类在循环中**: 通过策略配置保持控制权
4. **渐进增强**: 功能可选启用，向后兼容

### 技术选型

- **运行时**: Bun
- **CLI**: Commander.js
- **TUI**: Ink (React for CLI)
- **Web**: Hono + React + Vite
- **数据库**: SQLite (Bun native)
- **事件**: 内建事件总线 + SQLite 事件存储
- **任务队列**: 自建 (基于 SQLite)
