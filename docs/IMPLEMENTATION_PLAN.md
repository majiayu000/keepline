# Keepline Implementation Plan

> **策略**: 渐进式重构，保持向后兼容，每个 Phase 独立可测试可提交

---

## Phase 1: 基础重构 - 建立领域层结构

**目标**: 将现有代码重组为清晰的领域驱动架构，不改变功能

### 1.1 创建领域目录结构

```bash
# 新建目录
mkdir -p src/domain/{session,recovery}
mkdir -p src/infrastructure/{database,events,adapters}
mkdir -p src/application/use-cases
```

### 1.2 迁移 Session 领域

- [ ] 创建 `src/domain/session/entity.ts` - Session 实体定义
- [ ] 创建 `src/domain/session/value-objects.ts` - SessionId, SessionStatus
- [ ] 创建 `src/domain/session/events.ts` - 领域事件定义
- [ ] 创建 `src/domain/session/repository.ts` - 仓储接口
- [ ] 迁移 `session.service.ts` → `src/domain/session/service.ts`
- [ ] 迁移 `session.aggregator.ts` → `src/domain/session/aggregator.ts`

### 1.3 迁移 Recovery 领域

- [ ] 创建 `src/domain/recovery/entity.ts` - RecoveryAttempt 实体
- [ ] 创建 `src/domain/recovery/types.ts` - RecoveryMethod, RecoveryResult
- [ ] 迁移 `recovery.service.ts` → `src/domain/recovery/service.ts`

### 1.4 重构基础设施层

- [ ] 移动 `db/` → `src/infrastructure/database/`
- [ ] 移动 `adapters/` → `src/infrastructure/adapters/`
- [ ] 创建 `src/infrastructure/events/bus.ts` - 增强事件总线
- [ ] 创建 `src/infrastructure/events/store.ts` - 事件存储

### 1.5 创建数据库迁移系统

- [ ] 创建 `src/infrastructure/database/migrations/index.ts`
- [ ] 创建 `001_initial.ts` - 现有 schema
- [ ] 添加版本追踪表 `schema_migrations`

### 1.6 更新导入路径

- [ ] 更新所有 CLI 命令的导入
- [ ] 更新 Web API 路由的导入
- [ ] 更新测试文件的导入

### 1.7 测试 & 提交

- [ ] 运行 `bun run typecheck` - 类型检查通过
- [ ] 运行 `bun run test` - 所有测试通过
- [ ] 运行 `bun run build` - 构建成功
- [ ] 手动测试: `bun run dev list`, `bun run dev watch`
- [ ] Git commit: "refactor: reorganize codebase into domain-driven structure"

---

## Phase 2: 事件系统增强

**目标**: 实现持久化事件存储和事件驱动架构

### 2.1 增强事件总线

- [ ] 创建 `DomainEvent` 接口
- [ ] 创建 `EventBus` 类，支持:
  - 事件订阅/发布
  - 事件持久化到 SQLite
  - 异步处理器
  - 错误隔离

### 2.2 添加事件存储表

- [ ] 创建迁移 `002_events.ts`
- [ ] 添加 `events` 表

### 2.3 迁移现有事件

- [ ] 将现有 EventEmitter3 事件迁移到新系统
- [ ] 保持向后兼容 (双发布)

### 2.4 测试 & 提交

- [ ] 编写事件系统单元测试
- [ ] 运行所有测试
- [ ] Git commit: "feat: add persistent event store and enhanced event bus"

---

## Phase 3: 记忆系统 (Memory Domain)

**目标**: 实现会话上下文持久化，借鉴 Continuous Claude 的"接力赛"模式

### 3.1 创建 Memory 领域

- [ ] 创建 `src/domain/memory/entity.ts` - SessionMemory 实体
- [ ] 创建 `src/domain/memory/context-builder.ts` - 构建恢复上下文
- [ ] 创建 `src/domain/memory/extractor.ts` - 从 Hook 事件提取信息
- [ ] 创建 `src/domain/memory/service.ts` - 记忆服务

### 3.2 数据库迁移

- [ ] 创建迁移 `003_memory.ts`
- [ ] 添加 `session_memories` 表

### 3.3 集成到现有系统

- [ ] Hook 事件处理器: 自动更新记忆
- [ ] 恢复服务: 注入上下文选项

### 3.4 添加 CLI 命令

- [ ] 创建 `src/cli/commands/memory.ts`
- [ ] 实现 `keepline memory show <session-id>`
- [ ] 实现 `keepline memory edit <session-id>`

### 3.5 添加 API 端点

- [ ] `GET /api/memory/:sessionId`
- [ ] `PUT /api/memory/:sessionId`

### 3.6 测试 & 提交

- [ ] 编写记忆系统测试
- [ ] 测试 CLI 命令
- [ ] Git commit: "feat: add session memory system for context persistence"

---

## Phase 4: 自动恢复系统 (Recovery Enhancement)

**目标**: 实现策略驱动的自动恢复

### 4.1 增强 Recovery 领域

- [ ] 创建 `src/domain/recovery/policy.ts` - 恢复策略配置
- [ ] 创建 `src/domain/recovery/evaluator.ts` - 恢复前评估
- [ ] 创建 `src/domain/recovery/verifier.ts` - 恢复后验证
- [ ] 重构 `src/domain/recovery/service.ts` - 三阶段恢复

### 4.2 数据库迁移

- [ ] 创建迁移 `004_recovery.ts`
- [ ] 添加 `recovery_policies` 表
- [ ] 添加 `recovery_attempts` 表

### 4.3 实现自动恢复

- [ ] 事件处理器: SessionLost → 检查策略 → 触发恢复
- [ ] 重试逻辑: 指数退避
- [ ] 记忆集成: 恢复时注入上下文

### 4.4 添加 CLI 命令

- [ ] 创建 `src/cli/commands/policy.ts`
- [ ] 实现 `keepline policy add/list/enable/disable`
- [ ] 增强 `keepline recover --with-context`

### 4.5 添加 API 端点

- [ ] `GET/POST /api/recovery/policies`
- [ ] `GET /api/recovery/attempts`

### 4.6 测试 & 提交

- [ ] 编写恢复策略测试
- [ ] 测试自动恢复流程
- [ ] Git commit: "feat: add policy-driven auto-recovery system"

---

## Phase 5: 任务队列系统 (Task Domain)

**目标**: 实现任务队列和 Cron 调度

### 5.1 创建 Task 领域

- [ ] 创建 `src/domain/task/entity.ts` - Task 实体
- [ ] 创建 `src/domain/task/queue.ts` - 任务队列
- [ ] 创建 `src/domain/task/scheduler.ts` - Cron 调度器
- [ ] 创建 `src/domain/task/worker.ts` - 任务执行器
- [ ] 创建 `src/domain/task/service.ts` - 任务服务

### 5.2 数据库迁移

- [ ] 创建迁移 `005_tasks.ts`
- [ ] 添加 `tasks` 表
- [ ] 添加 `task_dependencies` 表

### 5.3 Daemon 集成

- [ ] 增强 `daemon.scheduler.ts` 支持任务队列
- [ ] 添加 Worker 进程管理

### 5.4 添加 CLI 命令

- [ ] 创建 `src/cli/commands/queue.ts`
- [ ] 创建 `src/cli/commands/schedule.ts`
- [ ] 实现 `keepline queue add/list/cancel`
- [ ] 实现 `keepline schedule add/list/enable`

### 5.5 添加 API 端点

- [ ] `GET/POST /api/tasks`
- [ ] `GET/POST /api/schedules`

### 5.6 测试 & 提交

- [ ] 编写任务队列测试
- [ ] 测试 Cron 调度
- [ ] Git commit: "feat: add task queue and cron scheduling system"

---

## Phase 6: 预算管理系统 (Budget Domain)

**目标**: 实现成本追踪和预算控制

### 6.1 创建 Budget 领域

- [ ] 创建 `src/domain/budget/entity.ts` - Budget 实体
- [ ] 创建 `src/domain/budget/tracker.ts` - 使用量追踪
- [ ] 创建 `src/domain/budget/service.ts` - 预算服务

### 6.2 数据库迁移

- [ ] 创建迁移 `006_budget.ts`
- [ ] 添加 `budgets` 表
- [ ] 添加 `usage_records` 表
- [ ] 添加 `daily_usage` 表

### 6.3 集成

- [ ] Hook 事件: 记录使用量
- [ ] 任务调度器: 预算感知

### 6.4 添加 CLI 命令

- [ ] 创建 `src/cli/commands/budget.ts`
- [ ] 实现 `keepline budget status/set/history`

### 6.5 添加 API 端点

- [ ] `GET/PUT /api/budget`
- [ ] `GET /api/budget/history`

### 6.6 测试 & 提交

- [ ] 编写预算系统测试
- [ ] Git commit: "feat: add budget tracking and cost control system"

---

## Phase 7: 告警系统 (Alert Domain)

**目标**: 实现多渠道通知

### 7.1 创建 Alert 领域

- [ ] 创建 `src/domain/alert/entity.ts` - Alert 实体
- [ ] 创建 `src/domain/alert/rule.ts` - 告警规则
- [ ] 创建 `src/domain/alert/channel.ts` - 告警渠道
- [ ] 创建 `src/domain/alert/service.ts` - 告警服务

### 7.2 实现渠道适配器

- [ ] 创建 `src/integrations/webhook/sender.ts`
- [ ] 创建 `src/integrations/slack/bot.ts` (可选)
- [ ] 创建 `src/integrations/discord/bot.ts` (可选)

### 7.3 数据库迁移

- [ ] 创建迁移 `007_alerts.ts`
- [ ] 添加 `alert_rules` 表
- [ ] 添加 `alert_channels` 表
- [ ] 添加 `alert_history` 表

### 7.4 添加 CLI 命令

- [ ] 创建 `src/cli/commands/alert.ts`
- [ ] 实现 `keepline alert add-channel/add-rule/test`

### 7.5 添加 API 端点

- [ ] `GET/POST /api/alerts/rules`
- [ ] `GET/POST /api/alerts/channels`
- [ ] `POST /api/alerts/test/:channelId`

### 7.6 测试 & 提交

- [ ] 编写告警系统测试
- [ ] 测试 Webhook 通知
- [ ] Git commit: "feat: add multi-channel alert notification system"

---

## Phase 8: 可观测性 (Metrics & Audit)

**目标**: 实现监控指标和审计日志

### 8.1 创建 Metrics 领域

- [ ] 创建 `src/domain/metrics/collector.ts`
- [ ] 创建 `src/domain/metrics/exporter.ts` - Prometheus 格式

### 8.2 创建 Audit 领域

- [ ] 创建 `src/domain/audit/logger.ts`
- [ ] 创建 `src/domain/audit/query.ts`

### 8.3 数据库迁移

- [ ] 创建迁移 `008_observability.ts`
- [ ] 添加 `audit_logs` 表

### 8.4 添加 API 端点

- [ ] `GET /api/metrics` - Prometheus 格式
- [ ] `GET /api/audit` - 审计日志查询

### 8.5 添加 CLI 命令

- [ ] 实现 `keepline metrics`
- [ ] 实现 `keepline audit`

### 8.6 测试 & 提交

- [ ] 编写可观测性测试
- [ ] Git commit: "feat: add metrics export and audit logging"

---

## 实施顺序

```
Phase 1 (基础重构)
    │
    ▼
Phase 2 (事件系统)
    │
    ├───────────────┬───────────────┐
    ▼               ▼               ▼
Phase 3         Phase 4         Phase 5
(记忆系统)      (自动恢复)      (任务队列)
    │               │               │
    └───────────────┴───────────────┘
                    │
                    ▼
              Phase 6 (预算)
                    │
                    ▼
              Phase 7 (告警)
                    │
                    ▼
              Phase 8 (可观测)
```

---

## 每个 Phase 的提交流程

```bash
# 1. 开发完成后
bun run typecheck    # 类型检查
bun run test         # 运行测试
bun run build        # 构建

# 2. 手动验证
bun run dev list
bun run dev watch
bun run dev status

# 3. 提交
git add .
git commit -m "feat/refactor: <描述>"
git push origin main
```

---

## 预估时间

| Phase | 内容 | 预估 |
|-------|------|------|
| 1 | 基础重构 | 2-3 小时 |
| 2 | 事件系统 | 1-2 小时 |
| 3 | 记忆系统 | 2-3 小时 |
| 4 | 自动恢复 | 3-4 小时 |
| 5 | 任务队列 | 3-4 小时 |
| 6 | 预算管理 | 2-3 小时 |
| 7 | 告警系统 | 2-3 小时 |
| 8 | 可观测性 | 2-3 小时 |

**总计**: ~18-25 小时

---

## 开始 Phase 1

准备好后，确认开始 Phase 1: 基础重构
