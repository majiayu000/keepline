# LanceDB Observation ID Injection Hardening Product Spec

Issue: https://github.com/majiayu000/keepline/issues/74

## Summary

Keepline 的 memory observation API 必须拒绝带有 filter DSL 注入字符的 observation id。用户或本地集成只能读取或删除明确命中的单条 observation，不能通过路径参数扩大到其它 observation 或整张 LanceDB 表。

## User Problem

Keepline 将跨会话记忆持久化到 LanceDB。当前按 observation id 读取和删除的路径把 HTTP 参数直接拼进 LanceDB filter 字符串。认证用户或本地恶意页面一旦能触达 API，就可能用带引号和布尔表达式的 id 读取额外数据或删除全部 observation。

用户需要的行为是 fail closed：非法 id 直接返回清晰的 400 错误，合法但不存在的 id 返回 404，底层向量库不应看到非法 filter。

## Product Behavior

1. `GET /api/memory/observations/:id` 对非法 observation id 返回 400。
2. `DELETE /api/memory/observations/:id` 对非法 observation id 返回 400。
3. 非法 observation id 包括空字符串、过短或过长字符串、空格、引号、SQL/filter 运算符片段、点号和其它不在允许字符集内的字符。
4. 合法 observation id 使用与 Keepline 生成的 UUID 兼容的安全字符集。
5. 合法但不存在的 observation id 保持现有 404 行为。
6. 底层 vector store 被直接调用时也必须拒绝非法 observation id，不能依赖 HTTP 路由作为唯一防线。
7. 此修复不改变 memory observation 的创建、搜索、sessionId 查询或 sessionId 批量删除行为。

## Non-Goals

- 不重写 LanceDB 查询层或替换向量数据库。
- 不改变 `sessionId` 的现有校验策略。
- 不实现 observation id 迁移或历史数据重写。
- 不处理 P2 中的 LanceDB 维度迁移、`count()` 性能或 session 批量删除性能。
- 不改变认证、JWT、rate limit 或 DNS-rebinding 防护。

## Acceptance Criteria

1. observation id 有单独的验证函数，允许 Keepline 当前生成的 UUID。
2. `GET /api/memory/observations/:id` 在非法 id 下返回 400，且不初始化或查询 LanceDB。
3. `DELETE /api/memory/observations/:id` 在非法 id 下返回 400，且不初始化或删除 LanceDB 数据。
4. `LanceDBVectorStore.getById()` 在非法 id 下抛出明确错误。
5. `LanceDBVectorStore.delete()` 在非法 id 下抛出明确错误。
6. `getById()` 和 `delete()` 不再使用未经校验的 path 参数构造 LanceDB filter。
7. 回归测试覆盖注入形 id，例如 `x' OR '1'='1`。

## Open Questions

- LanceDB 是否提供稳定的参数化 predicate API。如果没有，本次采用严格 id allowlist；后续可以在升级 LanceDB 时改为参数化查询。
