# LanceDB Observation ID Injection Hardening Task Plan

Issue: https://github.com/majiayu000/keepline/issues/74
Product spec: `specs/GH74/product.md`
Tech spec: `specs/GH74/tech.md`

## Tasks

### SP74-T1: Add observation id validation

Owner: implementation agent

Dependencies: GH74 issue evidence

Done when:

- `src/lib/observation-id.ts` exports `isValidObservationId()` and `assertValidObservationId()`.
- Valid UUID observation ids pass.
- Injection-style ids with quotes, spaces, or filter operators fail.
- API validation exports the helper for route and test reuse.

Verify:

```sh
bun test src/__tests__/validation.test.ts
```

### SP74-T2: Reject invalid ids at API boundary

Owner: implementation agent

Dependencies: SP74-T1

Done when:

- `GET /api/memory/observations/:id` returns 400 for invalid ids before vector store access.
- `DELETE /api/memory/observations/:id` returns 400 for invalid ids before vector store access.
- Valid but missing ids retain 404 behavior.

Verify:

```sh
bun test src/__tests__/observation-id.test.ts
```

### SP74-T3: Guard vector adapter direct calls

Owner: implementation agent

Dependencies: SP74-T1

Done when:

- `LanceDBVectorStore.getById()` rejects invalid ids before initialization/query.
- `LanceDBVectorStore.delete()` rejects invalid ids before initialization/delete.
- Invalid ids cannot reach LanceDB filter construction through these methods.

Verify:

```sh
bun test src/__tests__/observation-id.test.ts
```

### SP74-T4: Run deterministic verification and prepare PR

Owner: coordinator

Dependencies: SP74-T1, SP74-T2, SP74-T3

Done when:

- Focused tests pass.
- Typecheck passes.
- Full `bun test` passes or any unrelated failure is recorded with evidence.
- Build passes.
- PR references and closes #74 only if every acceptance criterion is satisfied.

Verify:

```sh
test -f specs/GH74/product.md
test -f specs/GH74/tech.md
test -f specs/GH74/tasks.md
bun run typecheck
bun test
bun run build
```
