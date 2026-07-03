# EventStore Lifecycle Tech Spec

Product spec: `specs/GH102/product.md`

Issue: https://github.com/majiayu000/keepline/issues/102

## Implementation Scope

- `src/infrastructure/events/store.ts`
- `src/infrastructure/events/index.ts`
- `src/infrastructure/database/migrations/001_initial.ts`
- `src/db/migrations.ts`
- `src/services/retention.service.ts`
- docs mentioning the optional `events` table
- focused tests

## Design

Choose the smaller safe decision after code audit. If no production caller is
available, remove the runtime EventStore surface and keep `hook_events`
unchanged. If retaining, wire a real lifecycle event such as session completion
or recovery into `EventStore` and keep retention cleanup aligned.

The implementation must not leave optional events reset behavior disconnected
from migrations.

## Verification

```sh
bun test src/__tests__/reset-database.test.ts src/__tests__/retention.service.test.ts
bun run typecheck
```

## Risks And Rollback

Removing EventStore can break downstream imports if it was public. Search first
and keep compatibility exports only if actively used.

