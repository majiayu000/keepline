# LanceDB Observation ID Injection Hardening Tech Spec

Issue: https://github.com/majiayu000/keepline/issues/74
Product spec: `specs/GH74/product.md`

## Current Behavior

- `src/web/api/routes/memory.ts` 的 `GET /observations/:id` 和 `DELETE /observations/:id` 直接读取 `c.req.param('id')`，没有格式校验。
- `src/infrastructure/vector/lancedb.adapter.ts` 在 `getById()` 中使用 `.where(\`id = '${id}'\`)`，在 `delete()` 中使用 `this.table.delete(\`id = '${id}'\`)`。
- `POST /observations` 使用 `crypto.randomUUID()` 创建 id，因此正常新数据的 id 形如 UUID，能被安全 allowlist 覆盖。
- `sessionId` 路径已有 `isValidSessionId()`，但 observation id 没有等价防线。

## Design

1. 新增共享 observation id 验证模块。

   Suggested file: `src/lib/observation-id.ts`

   Rules:

   - 类型必须是 `string`。
   - 长度 8 到 64。
   - 只允许 `a-z`、`A-Z`、`0-9`、`-`、`_`。
   - `crypto.randomUUID()` 生成的 UUID 必须通过。
   - 引号、空格、点号、分号、等号等字符必须失败。

2. API 路由 fail closed。

   `src/web/api/routes/memory.ts` 在 `GET /observations/:id` 和 `DELETE /observations/:id` 进入 vector store 之前调用 `isValidObservationId(id)`。失败时返回：

   ```json
   { "success": false, "error": "Invalid observation ID format" }
   ```

   HTTP status 为 400。

3. Vector adapter 加第二道防线。

   `LanceDBVectorStore.getById()` 和 `delete()` 在任何初始化或 LanceDB 调用之前执行 `assertValidObservationId(id)`。这样即使未来新增内部调用路径，也不会把非法 id 拼入 filter。

4. 保持合法 id 行为不变。

   本次不改变 LanceDB 的 filter DSL 形态，因为严格 allowlist 已保证被插值的 id 不含引号、空格或运算符。若 LanceDB 版本提供参数化 API，可作为后续强化替换。

## Affected Files

- `src/lib/observation-id.ts`
- `src/web/api/middleware/validation.ts`
- `src/web/api/routes/memory.ts`
- `src/infrastructure/vector/lancedb.adapter.ts`
- `src/__tests__/validation.test.ts`
- focused observation id / memory route regression tests
- `specs/GH74/product.md`
- `specs/GH74/tech.md`
- `specs/GH74/tasks.md`

## Verification Plan

Focused:

```sh
bun test src/__tests__/validation.test.ts
bun test src/__tests__/observation-id.test.ts
```

Repository:

```sh
bun run typecheck
bun test
bun run build
```

Workflow:

```sh
test -f specs/GH74/product.md
test -f specs/GH74/tech.md
test -f specs/GH74/tasks.md
```

`checks/check_workflow.py` is not present in this repository, so there is no repo-local SpecRail workflow checker to run for this tranche.

## Risks

- If historical observation ids used characters outside the allowlist, those rows would no longer be addressable by id through this endpoint. Current production creation path uses UUID, so this risk is acceptable.
- `deleteBySessionId()` still interpolates `sessionId`, but the existing API boundary already validates `sessionId` and the sessionId issue is outside GH74 scope. A future adapter-level `assertValidSessionId()` would be a separate hardening change.

## Rollback

Revert the GH74 commit. The API returns to prior permissive behavior. No migration or persisted data shape is changed.
