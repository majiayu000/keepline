# Agent Orchestrator Product Spec

Issue: https://github.com/majiayu000/keepline/issues/62

## Summary

Agent Orchestrator 让 Keepline 从“会话列表”升级为本地 agent runtime 的指挥视图。用户同时运行多个 Claude Code、Codex、未来 Cursor 或其他 agent session 时，应该能先看到“现在最该处理什么”，再进入具体 session、memory、work item 或 recovery 流程。

第一阶段不把 LLM 摘要作为必要条件。MVP 先用现有 session、usage、memory、workboard 和 activity 数据生成 deterministic Attention Queue，避免因为模型不可用、隐私未授权或 transcript 缺失而让核心体验失效。

## User Problem

1. 用户同时运行 3-10 个以上本地 agent sessions 时，终端和 dashboard 只能告诉用户“有哪些会话”，不能告诉用户“哪个最该看”。
2. waiting、lost、高成本、长时间 idle、blocked work item 等信号分散在不同界面，用户需要手动综合判断。
3. 直接引入云模型摘要会带来隐私和可靠性风险；本地模型不可用时不能让 Orchestrator 视图空白或误导。
4. 现有 Memory 和 Workboard 已经包含进度、handoff 和 blocked 信号，但缺少统一的 prioritization 层。

## Product Behavior

1. `keepline overview` 显示一个全局 Attention Queue，默认隐藏 completed sessions，按需要人工处理的优先级排序。
2. 排序必须 deterministic。相同输入产生相同 ranking、score、reason 列表和 recommended action。
3. Queue item 必须清楚标注来源：session status、usage/cost、memory、workboard、activity freshness 或 digest。
4. 没有数据就显示空白或低置信度，不得编造进度、阻塞点或完成状态。
5. waiting session 优先级高于 lost/recoverable session；lost/recoverable 高于高成本 session；高成本高于 stale/idle session。
6. 高成本信号只来自 persisted usage data。没有 usage data 时不能猜测成本。
7. lost/recoverable session 默认只显示最近恢复窗口内的候选项，避免陈旧丢失会话淹没 Queue；被隐藏的旧 lost sessions 必须在 stats 中计数，并可通过显式参数重新包含。
8. Memory 增强字段可以提供 `summary`、`nextActions`、`blockers`，但这些字段缺失时 Overview 仍可工作。
9. Session Digest 的第一版必须可区分 deterministic digest 和 model-generated digest。模型摘要不可覆盖用户手动 memory 数据。
10. 本地模型摘要默认本地优先；云模型必须显式配置，且 transcript 内容默认不离开本机。
11. 摘要生成失败必须显示 stale/error 状态或保留旧摘要，不得 warning 后静默显示假新摘要。
12. Web Orchestrator 视图应复用同一 Attention Queue API，不在前端重新实现排序规则。
13. 所有新 API 和 CLI 行为必须保持 runtime-neutral：Codex、Claude Code、未来 Cursor 都是 runtime/client 数据源，不是产品边界。

## MVP Scope

- 新增 deterministic Attention Queue 服务。
- 新增 `keepline overview` CLI。
- 新增只读 `GET /api/orchestrator/overview` API。
- 使用现有 sessions、usage stats、memory summaries 和 activity timestamps。
- 添加 SpecRail artifacts：`specs/GH62/product.md`、`tech.md`、`tasks.md`。
- 新增 focused tests 覆盖排序、reason、CLI/API contract。

## Follow-Up Scope

- Session Digest persistence and deterministic generator.
- Local model summarizer provider for Ollama / LM Studio.
- Web Orchestrator tab or Workboard integration.
- Semantic search and timeline playback.
- Cursor runtime adapter only after stable source contract is selected.

## Non-Goals

- 不在第一阶段调用云模型或上传 transcript。
- 不实现 Cursor adapter。
- 不重写现有 Sessions、Memory、WorkItems 或 Projects UI。
- 不让 agent 自动改变 WorkItem status。
- 不自动 resume、stop、merge、close 或删除任何 session。
- 不把 Overview ranking 作为 billing 或 hard enforcement 机制。

## Acceptance Criteria

1. `keepline overview` 在本地数据库有 sessions 时输出 Attention Queue。
2. `GET /api/orchestrator/overview` 返回 `{ success, data: { generatedAt, items, stats } }`。
3. 每个 item 至少包含 `sessionId`、`client`、`status`、`title`、`directory`、`score`、`rank`、`reasons`、`recommendedAction`、`lastActiveAt`。
4. waiting session 的 rank 高于 lost、high-cost、idle 和 completed session。
5. completed sessions 默认不进入 queue，除非调用方显式包含。
6. usage cost 只在 `usageStats.totalCost` 存在时产生 reason。
7. idle/stale reason 只根据 `lastActiveAt` 和配置阈值产生。
8. 输出 reason 不解析 free-text 来推断 done 或 blocked。
9. API route 需要沿用现有 auth middleware。
10. 新增代码不破坏 `list`、`watch`、`recover`、`memory`、`work-items`、`projects` 流程。
11. 默认 Overview 隐藏超过 lost freshness window 的 lost sessions，`stats.hiddenOldLost` 计数，显式 `includeOldLost=true` 可以包含它们。
12. 验证命令通过：`bun run typecheck`、targeted tests、`bun test`、`bun run build`。

## Open Questions

- P0 后是否把 `overview` 作为默认 dashboard 的第一屏，还是保持独立命令/API？
- 高成本阈值是否需要配置项，还是先固定默认值后续再开放？
- Session Digest 的模型 provider 配置是否放入现有 config，还是单独 `orchestrator` config section？
