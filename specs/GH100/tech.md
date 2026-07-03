# API And Web Contract Cleanup Tech Spec

Product spec: `specs/GH100/product.md`

Issue: https://github.com/majiayu000/keepline/issues/100

## Implementation Scope

- `src/web/client/src/types/session.ts`
- `src/web/client/src/types/api.ts`
- `src/web/client/src/types/orchestrator.ts`
- `src/web/client/src/hooks/useSessions.ts`
- `src/web/client/src/services/websocket.ts`
- `src/services/attention.prioritizer.ts`
- focused tests under `src/__tests__/`

## Design

Use existing TypeScript contracts first. Remove purely phantom fields when no
backend producer exists. For runtime boundary parsing, add small local predicate
helpers around WebSocket message handling and sessions API response handling so
unknown payloads are ignored or surfaced as explicit errors instead of becoming
undefined property access later.

`sync:complete` should keep the same wire event name but use a named handler in
`useSessions()` that refreshes sessions and stats.

## Verification

```sh
bun test src/__tests__/realtime-updates.test.ts src/__tests__/session-response.test.ts
bun run typecheck
```

## Risks And Rollback

The main risk is changing client-only TypeScript shape while the public API is
unchanged. Rollback is a pure client contract revert.

