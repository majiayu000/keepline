# High-Cost Notification Tech Spec

Product spec: `specs/GH78/product.md`

Issue: https://github.com/majiayu000/keepline/issues/78

## Context

- `src/web/client/src/hooks/useNotifications.ts` only evaluates the high-cost branch when `session.usageStats` exists.
- `src/web/client/src/hooks/useSessions.ts` fetches `api.fetchSessions('basic', ...)` for the dashboard list and notification snapshots.
- `src/web/api/server.ts` broadcasts `serializeBasicSessions(...)` over WebSocket.
- `src/web/api/session-response.ts` currently strips `usageStats` from the basic payload.
- `SessionRepository.findAllLightweight()` currently avoids heavy fields but also omits persisted usage columns.

## Design

Keep the basic payload lightweight, but include already-persisted usage columns:

```ts
usageStats?: {
  totalInputTokens: number
  totalOutputTokens: number
  totalTokens: number
  totalCost: number
  apiCalls: number
}
```

The database lightweight query should select `total_input_tokens`, `total_output_tokens`, `total_tokens`, `total_cost`, and `api_calls`. Mapping should match the full session mapper: return `usageStats` only when usage exists and use zero defaults for nullable parts.

`serializeBasicSession()` should include `usageStats` without adding heavy detail fields.

Realtime change detection should include stable usage fields in its state signature:

- `totalCost`
- `totalTokens`
- `apiCalls`

The client session version signature should include cost and token totals so `App` re-runs `checkSessionChanges` for cost-only changes.

For a focused notification regression, expose a small pure helper that derives notification events from previous and next sessions. The hook can call that helper and forward events to `notify()`.

## Failure Behavior

- Missing usage columns must serialize as no `usageStats`, not zero-cost guessed data.
- Cost updates must not require full session fetches.
- Notification derivation must not throw when a previous session lacks `usageStats`.

## Verification Plan

- `bun test src/__tests__/session-response.test.ts src/__tests__/sessions.route-basic.test.ts src/__tests__/use-notifications.test.ts`
- `bun run typecheck`
- `bun test`
- `bun run build`

## Rollback

- Remove usage fields from the lightweight query and basic serializer.
- Remove the client usage fields from the version signature and realtime state signature.
- Remove the notification helper test.
