# Runtime Identity Contract Tech Spec

Product spec: `specs/GH80/product.md`

Issue: https://github.com/majiayu000/keepline/issues/80

## Context

- `src/domain/runtime/types.ts` declares `RuntimeId`.
- `src/services/runtime-status.ts` declares a separate `SessionRuntimeId` and bridges runtime IDs to legacy `AgentClient`.
- `clientForRuntimeId()` currently uses a default branch that maps anything except `codex` to `claude`.
- `RuntimeId` includes `cursor` even though the default registry only registers Claude Code and Codex.

## Design

In `src/domain/runtime/types.ts`:

```ts
export const REGISTERED_RUNTIME_IDS = ['claude-code', 'codex'] as const
export type RegisteredRuntimeId = typeof REGISTERED_RUNTIME_IDS[number]
export type RuntimeId = RegisteredRuntimeId | (string & {})
```

In `src/services/runtime-status.ts`:

- Define `SessionRuntimeId = RegisteredRuntimeId`.
- Re-export `SESSION_RUNTIME_IDS = REGISTERED_RUNTIME_IDS`.
- Use compile-time checked maps:

```ts
const CLIENT_RUNTIME_IDS = {
  claude: 'claude-code',
  codex: 'codex',
} satisfies Record<AgentClient, SessionRuntimeId>

const RUNTIME_CLIENTS = {
  'claude-code': 'claude',
  codex: 'codex',
} satisfies Record<SessionRuntimeId, AgentClient>
```

- Add a small `isSessionRuntimeId()` guard.
- Make bridge functions throw on unknown inputs.
- Make `parseRuntimeFilter()` check the registered runtime ID set.

## Verification Plan

- Add `src/__tests__/runtime-status.test.ts`.
- Run:
  - `bun test src/__tests__/runtime-status.test.ts src/__tests__/session-runtime-scan.test.ts src/__tests__/sessions.route-basic.test.ts`
  - `bun run typecheck`
  - `bun test`
  - `bun run build`

## Rollback

- Restore duplicated `SessionRuntimeId` literal union and old bridge functions.
- Remove the runtime-status regression tests.
