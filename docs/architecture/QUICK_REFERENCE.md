# Keepline Quick Reference

## Architecture Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                      KEEPLINE V2                                   │
│              本地 Agent Runtime 总控层                            │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│    ┌──────┐  ┌──────┐  ┌──────┐  ┌──────┐  ┌──────┐            │
│    │ CLI  │  │ Web  │  │Slack │  │Discord│ │Webhook│           │
│    └──┬───┘  └──┬───┘  └──┬───┘  └──┬───┘  └──┬───┘            │
│       └─────────┴─────────┴─────────┴─────────┘                 │
│                           │                                      │
│                           ▼                                      │
│    ┌─────────────────────────────────────────────────────┐      │
│    │              APPLICATION LAYER                       │      │
│    │  Use Cases | Workflows | Plugins                     │      │
│    └─────────────────────────────────────────────────────┘      │
│                           │                                      │
│                           ▼                                      │
│    ┌─────────────────────────────────────────────────────┐      │
│    │                DOMAIN LAYER                          │      │
│    │                                                      │      │
│    │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐   │      │
│    │  │ Session │ │Recovery │ │  Task   │ │ Memory  │   │      │
│    │  └─────────┘ └─────────┘ └─────────┘ └─────────┘   │      │
│    │                                                      │      │
│    │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐   │      │
│    │  │ Budget  │ │  Alert  │ │  Audit  │ │ Metrics │   │      │
│    │  └─────────┘ └─────────┘ └─────────┘ └─────────┘   │      │
│    └─────────────────────────────────────────────────────┘      │
│                           │                                      │
│                           ▼                                      │
│    ┌─────────────────────────────────────────────────────┐      │
│    │            INFRASTRUCTURE LAYER                      │      │
│    │                                                      │      │
│    │  SQLite | EventBus | ProcessScanner | RuntimeAdapter │      │
│    │  HookServer | Terminal | Daemon | HTTP/WebSocket    │      │
│    └─────────────────────────────────────────────────────┘      │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## Core Domains

| 领域 | 职责 | 关键组件 |
|------|------|----------|
| **Session** | 会话生命周期管理 | Entity, Status, Events |
| **Recovery** | 自动恢复 | Policy, Strategy, Evaluator, Verifier |
| **Task** | 任务调度 | Queue, Scheduler, Worker |
| **Memory** | 上下文持久化 | ContextBuilder, Extractor, Handoff |
| **Budget** | 成本控制 | Tracker, Limits, SchedulerAware |
| **Alert** | 多渠道通知 | Rules, Channels, Throttle |
| **Audit** | 审计日志 | Logger, Query, Retention |
| **Metrics** | 可观测性 | Collector, Exporter |

---

## Key Data Flows

### 1. Session Discovery Flow
```
ps + lsof → ProcessScanner → SessionService → SQLite
     ↓
Runtime logs → RuntimeAdapter → SessionService → Events
```

### 2. Auto Recovery Flow
```
SessionLost Event
     ↓
RecoveryPolicy Match
     ↓
┌────────────┐    ┌────────────┐    ┌────────────┐
│  Evaluate  │ →  │  Execute   │ →  │   Verify   │
│  (只读分析) │    │  (执行恢复) │    │  (验证成功) │
└────────────┘    └────────────┘    └────────────┘
     ↓                                    ↓
Memory.getContext()              Memory.injectContext()
```

### 3. Task Scheduling Flow
```
Cron Expression → TaskScheduler → TaskQueue → TaskWorker
                                      ↓
                        BudgetTracker.shouldAllow()
                                      ↓
                              Claude CLI Execute
```

---

## Database Tables

```sql
-- Core
sessions              -- 会话主表
session_memories      -- 会话记忆 [NEW]

-- Task System
tasks                 -- 任务表 [NEW]
task_dependencies     -- 任务依赖 [NEW]

-- Recovery System
recovery_policies     -- 恢复策略 [NEW]
recovery_attempts     -- 恢复记录 [NEW]

-- Budget System
budgets               -- 预算配置 [NEW]
usage_records         -- 使用记录 [NEW]
daily_usage           -- 每日汇总 [NEW]

-- Alert System
alert_rules           -- 告警规则 [NEW]
alert_channels        -- 告警渠道 [NEW]
alert_history         -- 告警历史 [NEW]

-- Infrastructure
events                -- 事件存储 [NEW]
hook_events           -- Hook 事件
audit_logs            -- 审计日志 [NEW]
metadata              -- 元数据
workflows             -- 工作流 [NEW]
workflow_runs         -- 工作流运行 [NEW]
```

---

## CLI Commands Summary

```bash
# 会话
keepline list                    # 列出会话
keepline watch                   # 实时监控
keepline recover <id>            # 恢复会话 (--with-context)

# 任务 [NEW]
keepline queue add --prompt "..." --directory "..."
keepline queue status
keepline schedule add --cron "0 8 * * *" --prompt "..."

# 记忆 [NEW]
keepline memory show <session-id>
keepline memory edit <session-id>

# 预算 [NEW]
keepline budget status
keepline budget set --daily 10

# 恢复策略 [NEW]
keepline policy add --name "快速恢复" --method resume
keepline policy enable <id>

# 告警 [NEW]
keepline alert add-channel --type slack --webhook "..."
keepline alert add-rule --event session_lost --severity critical

# 工作流 [NEW]
keepline workflow run daily-review ./my-project

# 系统
keepline daemon start --scheduler
keepline status
keepline metrics
```

---

## API Endpoints Summary

```
Sessions:     GET/POST /api/sessions, /api/sessions/:id/recover
Tasks:        GET/POST /api/tasks, /api/tasks/:id/cancel
Schedules:    GET/POST /api/schedules, /api/schedules/:id/enable
Memory:       GET/PUT  /api/memory/:sessionId
Budget:       GET/PUT  /api/budget, /api/budget/history
Policies:     GET/POST /api/recovery/policies
Alerts:       GET/POST /api/alerts/rules, /api/alerts/channels
Metrics:      GET      /api/metrics (Prometheus format)
Workflows:    GET/POST /api/workflows, /api/workflows/:id/run
Audit:        GET      /api/audit
```

---

## Event Types

```typescript
// Session Events
'session.discovered' | 'session.updated' | 'session.lost' | 'session.recovered'

// Task Events
'task.queued' | 'task.started' | 'task.completed' | 'task.failed'

// Budget Events
'budget.warning' | 'budget.exceeded'

// Recovery Events
'recovery.started' | 'recovery.completed' | 'recovery.failed'

// Alert Events
'alert.triggered' | 'alert.sent'
```

---

## Configuration

```json
{
  "scanInterval": 5000,
  "hookPort": 7890,
  "recovery": {
    "enabled": true,
    "defaultPolicy": "quick-recovery"
  },
  "tasks": {
    "maxConcurrency": 2,
    "defaultTimeout": 300000
  },
  "budget": {
    "enabled": true,
    "limits": { "daily": 10, "monthly": 300 },
    "alerts": { "warningThreshold": 0.8, "pauseThreshold": 0.95 }
  },
  "alerts": {
    "enabled": true,
    "defaultCooldown": 300
  }
}
```

---

## Implementation Phases

| Phase | Focus | Duration |
|-------|-------|----------|
| 1 | 基础重构 (分层架构) | 1-2 周 |
| 2 | 记忆系统 | 1 周 |
| 3 | 自动恢复 | 1-2 周 |
| 4 | 任务队列 | 1-2 周 |
| 5 | 预算管理 | 1 周 |
| 6 | 告警系统 | 1 周 |
| 7 | 可观测性 | 1 周 |
| 8 | 工作流 & 插件 | 2 周 |

---

## Design Principles

1. **接力赛模式** - 每次迭代只做一件事，留笔记给下次
2. **容错优先** - 失败是正常的，自动重试和降级
3. **人类在循环中** - 通过策略配置保持控制权
4. **渐进增强** - 功能可选启用，向后兼容

---

## Borrowed Ideas

| Source | Idea | Implementation |
|--------|------|----------------|
| **Continuous Claude** | 接力赛模式 | Memory Domain |
| **Continuous Claude** | SHARED_TASK_NOTES.md | SessionMemory Entity |
| **Sleepless Agent** | 三阶段工作流 | Evaluator → Executor → Verifier |
| **Sleepless Agent** | 预算感知调度 | BudgetTracker.shouldAllow() |
| **Sleepless Agent** | Slack 集成 | Alert Channels |
