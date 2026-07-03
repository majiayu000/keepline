# EventStore Lifecycle Product Spec

Issue: https://github.com/majiayu000/keepline/issues/102

## Summary

Keepline should either use EventStore as a real production event persistence
surface or remove the unused table and exports from runtime setup.

## Product Behavior

1. The repository must document whether EventStore is retained or removed.
2. If retained, at least one production path must persist a real domain event.
3. If removed, migrations, reset, exports, and docs must stop exposing the dead
   table.
4. Tests must prove the chosen lifecycle decision.

## Non-Goals

- Do not introduce event sourcing for every domain.
- Do not change `hook_events`; that table has a separate runtime purpose.

## Acceptance Criteria

1. EventStore declarations, migrations, exports, and callers are audited.
2. A single retain/remove decision is implemented.
3. The selected path has focused tests.
4. Reset and export surfaces no longer contradict the decision.

