# Agent Orchestrator Tech Spec

Product spec: `specs/GH62/product.md`

Issue: https://github.com/majiayu000/keepline/issues/62

## Context

- `src/services/session.aggregator.ts` 已经提供 full/basic aggregated sessions 和 live process status。
- `src/domain/session/value-objects.ts` 已经定义 `needsAttention(status)`，但只覆盖 waiting/lost，没有 ranking 或 reason。
- `src/domain/memory/entity.ts` 和 `src/services/memory.service.ts` 已经有 relay-race memory 字段，可作为 digest 后续输入。
- `src/domain/work-item/workboard.ts` 已经有 waiting/stale/now/done 投影规则，Orchestrator 不应复制 WorkItem 状态流转。
- `src/web/api/routes/sessions.ts`、`projects.ts`、`work-items.ts` 已经采用 Hono route module + auth middleware 结构。
- `src/cli/index.ts` 使用 Commander 注册命令；`list.tsx` 会先 run migrations + sync sessions。
- `specs/GH44/product.md` 和 `tech.md` 要求 runtime-neutral orchestration，不允许把 Claude Code 或 Codex 写成系统边界。

## Design

新增一个只读 application service：

```ts
interface AttentionReason {
  code: 'waiting_for_human' | 'recoverable_lost' | 'high_cost' | 'stale_activity' | 'idle_activity' | 'active_session'
  severity: 'critical' | 'warning' | 'info'
  message: string
  score: number
}

interface AttentionQueueItem {
  rank: number
  sessionId: string
  client: AgentClient
  status: SessionStatus
  title: string
  directory: string
  lastActiveAt: Date
  score: number
  reasons: AttentionReason[]
  recommendedAction: 'review' | 'recover' | 'monitor' | 'resume' | 'none'
  processRunning: boolean
  usageCost?: number
}
```

Suggested module: `src/services/attention.prioritizer.ts`.

The service accepts sessions plus optional thresholds and returns:

```ts
interface AttentionOverview {
  generatedAt: Date
  items: AttentionQueueItem[]
  stats: {
    totalCandidates: number
    needingAttention: number
    critical: number
    warning: number
  }
}
```

## Ranking Rules

Rules are tiered and deterministic. A lower-priority reason must not stack with
other lower-priority reasons to outrank a higher-priority primary reason:

1. `waiting`: `waiting_for_human`, critical, base score 1000, action `review`.
2. `lost`: `recoverable_lost`, critical, base score 850, action `recover`.
3. `usageStats.totalCost >= highCostThreshold`: `high_cost`, warning, score 600 plus bounded cost bonus, action remains current higher-priority action or `review`.
4. `lastActiveAt` older than stale threshold and status is `running` or `idle`: `stale_activity`, warning, score 350, action `review`.
5. `idle`: `idle_activity`, info, score 150, action `monitor`.
6. `running`: `active_session`, info, score 25, action `monitor`.

Tie-breakers:

1. higher primary reason score first
2. more recent `lastActiveAt` first
3. `sessionId` ascending

Completed sessions are excluded by default.

## API

Add route module `src/web/api/routes/orchestrator.ts`:

- `GET /api/orchestrator/overview`
- query params:
  - `includeCompleted=true|false`
  - `limit=<number>` default 20, max 100
  - `highCostThreshold=<number>` optional, default from service
  - `staleHours=<number>` optional, default from service

Mount it in `src/web/api/routes/index.ts` and `src/web/api/server.ts`.

Serialize dates to ISO strings. Do not expose raw internal class instances.

## CLI

Add `src/cli/overview.ts` and register:

```sh
keepline overview
keepline overview --all
keepline overview --limit 10
keepline overview --json
```

The CLI should:

1. run migrations
2. sync sessions
3. build overview from `getAggregatedSessions()`
4. output a compact table by default
5. output JSON when `--json` is passed

## Data Model

PR1 does not require a migration.

Future PR for Session Digest may add a dedicated `session_digests` table rather than overloading `session_memories`. That follow-up must preserve existing Memory fields and distinguish deterministic/model-generated summary sources.

## Privacy and Failure Behavior

- PR1 does not read transcript content directly and does not call models.
- Missing usage data means no high-cost reason.
- Bad query params return 400 rather than silently falling back when the value is invalid.
- Any API failure returns 500 with a generic message and logs the error.

## Verification Plan

- Unit test `buildAttentionOverview()` ordering and reason generation.
- Route test `/api/orchestrator/overview` auth + serialization + limit.
- CLI smoke test can be covered by typecheck/build in PR1; a later CLI test harness can add subprocess coverage.
- Run:
  - `bun test src/__tests__/attention.prioritizer.test.ts src/__tests__/orchestrator.route.test.ts`
  - `bun run typecheck`
  - `bun test`
  - `bun run build`

## Rollback

- Remove the new route mount and CLI command registration.
- Remove `src/services/attention.prioritizer.ts`, `src/cli/overview.ts`, and related tests.
- No database rollback is needed for PR1 because no migration is introduced.
