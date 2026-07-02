# Runtime Identity Contract Task Plan

Issue: https://github.com/majiayu000/keepline/issues/80

## Tasks

### SP80-T1: Add SpecRail artifacts

Done when:

- `specs/GH80/product.md` exists.
- `specs/GH80/tech.md` exists.
- `specs/GH80/tasks.md` exists.

Verify:

```sh
test -f specs/GH80/product.md
test -f specs/GH80/tech.md
test -f specs/GH80/tasks.md
```

### SP80-T2: Move registered runtime IDs to runtime domain

Writable files:

- `src/domain/runtime/types.ts`

Done when:

- Registered runtime IDs are exported as a const tuple.
- `RuntimeId` derives from registered IDs plus future extension strings.
- The dead `cursor` registered literal is removed.

Verify:

```sh
bun run typecheck
```

### SP80-T3: Make runtime-status bridges explicit

Writable files:

- `src/services/runtime-status.ts`

Done when:

- `SessionRuntimeId` derives from `RegisteredRuntimeId`.
- `runtimeIdForClient()` does not default unknown clients to Claude.
- `clientForRuntimeId()` does not default unknown runtime IDs to Claude.
- `parseRuntimeFilter()` checks registered runtime IDs.

Verify:

```sh
bun test src/__tests__/runtime-status.test.ts
```

### SP80-T4: Add regression tests

Writable files:

- `src/__tests__/runtime-status.test.ts`

Done when:

- Known mappings still pass.
- Unknown runtime IDs throw instead of returning `claude`.
- Unknown clients throw instead of returning `claude-code`.
- Runtime filters reject unregistered IDs.

Verify:

```sh
bun test src/__tests__/runtime-status.test.ts
```

### SP80-T5: Full verification and PR

Done when:

- Focused tests pass.
- Typecheck passes.
- Full tests pass.
- Build passes.
- PR links and closes #80.

Verify:

```sh
bun test src/__tests__/runtime-status.test.ts src/__tests__/session-runtime-scan.test.ts src/__tests__/sessions.route-basic.test.ts
bun run typecheck
bun test
bun run build
```
