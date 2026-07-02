# Retention Cleanup Tech Spec

Product spec: `specs/GH81/product.md`

Issue: https://github.com/majiayu000/keepline/issues/81

## Context

- `config.retentionDays` defaults to 30 and validates as non-negative.
- `sessionRepository.deleteOldSessions(retentionDays)` exists but is not called.
- `eventStore.deleteOlderThan(date)` exists but is not called.
- `tool_usage` has a foreign key to `sessions(session_id)`, so old session deletion must remove child usage rows first.
- `hook_events` has `session_id` but no foreign key; it should still be cleaned for deleted sessions.

## Design

Add `src/services/retention.service.ts`:

```ts
interface RetentionCleanupResult {
  disabled: boolean
  retentionDays: number
  cutoff?: Date
  sessionsDeleted: number
  eventsDeleted: number
}
```

`runRetentionCleanup(retentionDays = config.get().retentionDays, now = new Date())`:

1. If `retentionDays <= 0`, return a disabled result without touching storage.
2. Compute `cutoff = now - retentionDays`.
3. Call `sessionRepository.deleteOldSessions(retentionDays, now)`.
4. Call `eventStore.deleteOlderThan(cutoff)`.
5. Log the cleanup result.

Update `sessionRepository.deleteOldSessions()` to run in one transaction:

1. Count completed sessions whose `last_active_at` is older than cutoff.
2. Delete `tool_usage` rows for those session IDs.
3. Delete `hook_events` rows for those session IDs.
4. Delete the session rows.
5. Return the deleted session count.

Update `daemon.scheduler.ts`:

- Add a daily retention interval.
- Run one cleanup cycle after the initial scan.
- Schedule recurring cleanup only when startup config has `retentionDays > 0`.
- Clear the interval on scheduler stop.

## Verification Plan

- Add `src/__tests__/retention.service.test.ts`:
  - old completed sessions and related usage/hook rows are removed
  - recent completed sessions and old non-completed sessions are preserved
  - old event-store rows are removed
  - `retentionDays <= 0` is a no-op
- Run:
  - `bun test src/__tests__/retention.service.test.ts src/__tests__/session-repository.test.ts`
  - `bun run typecheck`
  - `bun test`
  - `bun run build`

## Rollback

- Remove the scheduler retention interval.
- Remove `retention.service.ts`.
- Restore `deleteOldSessions()` to only delete sessions.
- Remove retention tests.
