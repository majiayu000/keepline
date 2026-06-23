# Runtime-Neutral Agent Orchestration Product Spec

Issue: https://github.com/majiayu000/claude-hub/issues/44

## Summary

Agent Runtime Hub 应该是本地 coding-agent runtime 和用户工作项的总控层，而不是 Codex、Claude Code、Cursor 或任一具体客户端的包装器。

Codex 和 Claude Code 是首批 runtime adapter。后续 Cursor、IDE agent、云端 agent 或其他 CLI/cloud agent 应通过同一个 runtime contract 接入。用户打开 dashboard 时看到的是项目、工作项、agent session、状态、证据、恢复、用量和上下文，而不是被迫理解某一个 runtime 的内部文件格式。

Workboard 是 Todo + runtime session + evidence 的投影视图，不是唯一任务入口。用户可以先捕获 Inbox/Todo/Idea，再把 runtime sessions 作为执行证据挂到工作项上。

## User Problem

高级用户会同时运行多个 agent runtime：例如 Claude Code 负责长上下文重构，Codex 负责快速本地编辑，未来 Cursor 或 IDE agent 负责编辑器内协作。如果 hub 的数据模型和 UI 仍以单一 runtime 为中心，用户会遇到这些问题：

1. 不同 runtime 的 session 无法在同一个项目或工作项视图里比较。
2. 某个 runtime parser 失败会污染或隐藏其他 runtime 的健康状态。
3. 新 runtime 接入需要改动 Claude-specific 或 Codex-specific 代码，长期会变成分支判断堆叠。
4. 文案和 API 暗示产品只服务一个 runtime，导致用户不确定 Codex、Claude Code、Cursor 是否是一等公民。
5. 工作项、runtime session、文件证据和进度状态没有清晰关系时，UI 很容易把推断当事实。
6. agent 如果能静默推进 manual task 状态，用户会失去对 planned/done/blocked 的信任。
7. 恢复命令如果用 shell 字符串拼接，后续支持更多 runtime 时会放大命令注入和参数转义风险。

## Product Behavior

1. 产品边界是 runtime-neutral orchestration hub。Codex、Claude Code、Cursor 等是数据源和执行入口，不是系统边界。

2. 用户可以先捕获 Inbox、Todo 或 Idea，不强制选择 project。后续可以手动关联 project、area、runtime session 或 evidence。

3. planned、done、blocked 这类正式工作项状态只能由用户直接设置，或由用户接受 agent 建议后设置。agent 不能根据聊天或文件推断静默改变 manual task 状态。

4. AgentSession 是证据层。Claude Code、Codex、未来 Cursor 都通过 adapter 产生 normalized session；UI 和 Workboard 只依赖 runtimeId、project、status、activity 和 evidence。

5. 每个 session 必须带有明确的 runtimeId。首批稳定 ID 是 claude-code 和 codex。未来 Cursor 使用 cursor，其他 runtime 使用稳定小写 ID。

6. dashboard、CLI、API 和 specs 中展示 runtime identity 时，应使用 runtime badge、filter 或 metadata，而不是把某个 runtime 写死为全局默认。

7. 如果一个 project 同时有 Claude Code 和 Codex sessions，项目卡、session 列表和详情视图应该能区分来源，并显示每个 runtime 的状态与数量。

8. 某个 runtime 的 session 根目录不存在时，如果这是正常情况，应静默 no-op 或 debug 记录。真实读取/解析错误必须保留在该 runtime 的错误结果中，不得让其他 runtime 的 sessions 消失。

9. 进度必须可追溯。没有数据就显示空白；推断进度必须标注来源；错误或缺失数据不得伪装成完成。

10. Project identity 应使用 git root、cwd 或 Unknown project，不能按 basename 合并。Project filter 必须是 exact project root filter，而不是文本搜索。

11. 恢复 session 的用户体验应保留 runtime-specific 行为，但底层 contract 返回结构化 command，例如 executable、args、cwd，而不是 shell command string。

12. 现有安装兼容入口可以保留。是否更改 package/bin/data path 是单独发布决策，不应阻塞 runtime-neutral architecture。

13. 文案应避免 Codex Hub 或 Claude-only session monitor 这类系统边界表达。可以在兼容性说明、历史迁移、具体 adapter 名称中保留 runtime 名称。

## MVP Scope

- 定义 runtime-neutral WorkItem、AgentSession、ProgressEvidence 和 runtime adapter contracts。
- 支持 Inbox/Todo/Idea 的基础 capture 和人工状态流转。
- 定义 Workboard 的 Now、Waiting、Stale 等投影规则。
- 将现有 Claude Code scanner 包装为 ClaudeCodeRuntimeAdapter。
- 新增或整理 Codex rollout JSONL adapter。
- 新增 RuntimeRegistry，支持注册、查询和扫描多个 adapters。
- 新增 runtime session 到 work item 的链接模型。
- 定义 project identity 和 exact project root filtering。
- 给 Codex parser 增加缺目录 no-op 和坏文件隔离测试。
- 更新关键 docs/UI/package 文案，使产品语义不局限于 Codex 或 Claude Code。
- 记录后续 Cursor adapter 的接入点，但不实现 Cursor。

## Non-Goals

- 不做团队协作、cloud sync、Jira 替代或权限系统。
- 不做 Warp tab 检测或自动改名。
- 不让 agent 自动从 todo 改代码；执行仍需要用户明确发起。
- 不把 Codex 或 Claude Code 变成产品边界。
- 不重写 Claude parser；第一阶段只包进 adapter。
- 不在本 spec 中强制决定最终品牌名、package name 或 CLI bin name。
- 不删除现有兼容环境变量、数据目录或迁移路径。
- 不实现完整 Cursor runtime。
- 不重写所有历史文档；历史迁移、已关闭 issue、测试名称可以保留旧名称。
- 不在没有用户授权的情况下 merge PR。

## Acceptance Criteria

1. 存在 checked-in SpecRail artifacts：specs/GH44/product.md 和 specs/GH44/tech.md。
2. Product/tech spec 明确区分 WorkItem、AgentSession、ProgressEvidence 和 runtime adapter。
3. 代码层有可复用的 AgentRuntimeAdapter contract，包含 descriptor、scanSessions() 和可选 buildRecoveryCommand(session, RuntimeRecoveryOptions)。
4. RuntimeSession 至少包含 runtimeId、sessionId、cwd、status、title、lastActiveAt、toolCount、messageCount 和 runtime metadata。
5. 默认 registry 注册 claude-code 和 codex。
6. Codex rollout JSONL fixture 能解析为 normalized session。
7. Codex 缺 sessions 目录是 no-op；坏 JSONL 文件会进入 runtime scan errors，不会隐藏同目录下其他好 session。
8. resume command contract 使用结构化参数，避免 shell string 拼接。
9. 无项目 capture、agent 建议不自动转正式 todo、无 evidence 不显示假进度。
10. Project filter 使用 exact project root，并覆盖 search/status/runtime/pagination 组合。
11. 关键文案从 runtime-specific 产品边界改为 runtime-neutral hub 语义。
12. 验证命令通过：bun run typecheck、targeted tests、bun test、bun run build。

## Related Work

- #34 曾覆盖 Keepline rename + Codex detection，但已关闭，且方向不同。
- #38 只覆盖 missing Codex sessions directory no-op，可作为 runtime adapter 验收子项。
- #41/#43 覆盖 Project Workspaces，不覆盖 runtime-neutral orchestration foundation。

## Open Questions

- 最终产品名是否继续使用当前兼容名，还是独立发布为新 bin/package？
- Work items 第一版是否需要 SQLite migration，还是先以现有 session memory 表旁路验证 UX？
- RuntimeRegistry.scanAll() 接入现有 sessions API 时，是否保留 legacy aggregation endpoint？
- Runtime parser errors 应该只出现在 diagnostics/log，还是也在 UI 中显示 per-runtime degraded state？
- Cursor runtime 的 session source 是 IDE state、local logs、extension API，还是用户配置路径？
