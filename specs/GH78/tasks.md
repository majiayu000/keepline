# High-Cost Notification Task Plan

Issue: https://github.com/majiayu000/keepline/issues/78

## Tasks

### SP78-T1: Add SpecRail artifacts

Done when:

- `specs/GH78/product.md` exists.
- `specs/GH78/tech.md` exists.
- `specs/GH78/tasks.md` exists.

Verify:

```sh
test -f specs/GH78/product.md
test -f specs/GH78/tech.md
test -f specs/GH78/tasks.md
```

### SP78-T2: Preserve usage data in lightweight backend sessions

Writable files:

- `src/domain/session/entity.ts`
- `src/infrastructure/database/repositories/session.repository.ts`
- `src/web/api/session-response.ts`

Done when:

- Lightweight repository rows include persisted usage columns.
- `SessionListItem` can carry `usageStats`.
- `serializeBasicSession()` emits `usageStats` when available and still omits heavy fields.

Verify:

```sh
bun test src/__tests__/session-response.test.ts src/__tests__/sessions.route-basic.test.ts
```

### SP78-T3: Make realtime and client versioning cost-aware

Writable files:

- `src/web/api/server.ts`
- `src/web/client/src/hooks/useSessions.ts`

Done when:

- Realtime broadcast state changes when usage cost or token totals change.
- Client session version signature changes when usage cost or token totals change.

Verify:

```sh
bun run typecheck
```

### SP78-T4: Add high-cost notification regression

Writable files:

- `src/web/client/src/hooks/useNotifications.ts`
- `src/__tests__/use-notifications.test.ts`

Done when:

- A mocked previous/new session pair crossing the threshold produces one high-cost notification event.
- A session already above the threshold does not produce another crossing event.

Verify:

```sh
bun test src/__tests__/use-notifications.test.ts
```

### SP78-T5: Full local verification and PR

Done when:

- Focused tests pass.
- Typecheck passes.
- Full test suite passes.
- Build passes.
- PR links and closes #78.

Verify:

```sh
bun test src/__tests__/session-response.test.ts src/__tests__/sessions.route-basic.test.ts src/__tests__/use-notifications.test.ts
bun run typecheck
bun test
bun run build
```
