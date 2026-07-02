# Retention Cleanup Product Spec

Issue: https://github.com/majiayu000/keepline/issues/81

## Summary

Keepline's configured retention policy must actually delete old persisted data while the daemon is running. A default 30-day retention setting is currently declared but never executed.

## User Problem

Users are told that completed session data is retained for a bounded number of days, but completed sessions and related rows remain forever. Long-running daemon installations will grow the SQLite database indefinitely.

## Product Behavior

1. The daemon runs retention cleanup on startup and then periodically.
2. `retentionDays > 0` enables cleanup.
3. `retentionDays <= 0` disables cleanup.
4. Cleanup deletes completed sessions older than the configured retention window.
5. Cleanup deletes `tool_usage` and `hook_events` rows associated with deleted sessions.
6. Cleanup deletes old domain events from the event store using the same cutoff.
7. Active, waiting, idle, lost, or recent completed sessions are preserved.

## Non-Goals

- Do not delete active session history only because it is old.
- Do not add a new user-facing config key.
- Do not change the default retention value.
- Do not vacuum or compact the SQLite database in this tranche.

## Acceptance Criteria

1. Daemon startup runs one retention cleanup cycle.
2. Daemon schedules periodic retention cleanup when `retentionDays > 0`.
3. `retentionDays <= 0` disables cleanup.
4. Old completed sessions are deleted.
5. Related `tool_usage` and `hook_events` rows for deleted sessions are deleted.
6. Old event-store rows are deleted.
7. Regression tests insert old records and prove cleanup removes only eligible data.
8. Focused tests, `bun run typecheck`, `bun test`, and `bun run build` pass.
