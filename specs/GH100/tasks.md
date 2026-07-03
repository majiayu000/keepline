# API And Web Contract Cleanup Task Plan

Issue: https://github.com/majiayu000/keepline/issues/100

Product spec: `specs/GH100/product.md`
Tech spec: `specs/GH100/tech.md`

## Tasks

### SP100-T1: Remove or wire dead contract fields

Done when frontend types match backend producers for session and orchestrator
payloads.

Verify:

```sh
bun run typecheck
```

### SP100-T2: Centralize critical client payload parsing

Done when WebSocket and sessions payload handling uses named guards for the
fields it consumes.

Verify:

```sh
bun test src/__tests__/realtime-updates.test.ts
```

### SP100-T3: Preserve behavior with focused tests

Verify:

```sh
bun test src/__tests__/session-response.test.ts src/__tests__/realtime-updates.test.ts
bun run typecheck
```

