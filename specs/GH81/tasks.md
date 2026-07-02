# Retention Cleanup Task Plan

Issue: https://github.com/majiayu000/keepline/issues/81

## Tasks

### SP81-T1: Add SpecRail artifacts

Done when:

- `specs/GH81/product.md` exists.
- `specs/GH81/tech.md` exists.
- `specs/GH81/tasks.md` exists.

Verify:

```sh
test -f specs/GH81/product.md
test -f specs/GH81/tech.md
test -f specs/GH81/tasks.md
```

### SP81-T2: Make old session deletion complete

Writable files:

- `src/infrastructure/database/repositories/session.repository.ts`

Done when:

- Old completed sessions are deleted in a transaction.
- Matching `tool_usage` rows are deleted before sessions.
- Matching `hook_events` rows are deleted.
- Running or recent sessions are preserved.

Verify:

```sh
bun test src/__tests__/retention.service.test.ts src/__tests__/session-repository.test.ts
```

### SP81-T3: Add retention service and scheduler wiring

Writable files:

- `src/services/retention.service.ts`
- `src/services/daemon.scheduler.ts`

Done when:

- Cleanup can be called directly for tests.
- Scheduler runs cleanup once on startup.
- Scheduler schedules recurring cleanup when `retentionDays > 0`.
- Scheduler clears the retention interval on stop.

Verify:

```sh
bun run typecheck
```

### SP81-T4: Add retention regression tests

Writable files:

- `src/__tests__/retention.service.test.ts`

Done when:

- Old completed session rows are deleted.
- Related usage and hook event rows are deleted.
- Recent completed and old running sessions remain.
- Old event-store rows are deleted.
- `retentionDays <= 0` does not delete data.

Verify:

```sh
bun test src/__tests__/retention.service.test.ts
```

### SP81-T5: Full verification and PR

Done when:

- Focused tests pass.
- Typecheck passes.
- Full tests pass.
- Build passes.
- PR links and closes #81.

Verify:

```sh
bun test src/__tests__/retention.service.test.ts src/__tests__/session-repository.test.ts
bun run typecheck
bun test
bun run build
```
