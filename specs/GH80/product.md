# Runtime Identity Contract Product Spec

Issue: https://github.com/majiayu000/keepline/issues/80

## Summary

Keepline must not silently coerce unknown agent runtimes into Claude. Runtime identity should have one registered runtime-id source, and legacy session `client` values should be mapped through explicit, checked bridges.

## User Problem

The codebase currently has separate runtime/client vocabularies. The highest-risk behavior is that unknown runtime IDs can fall through bridge functions and become `claude`, which hides integration mistakes and mislabels future runtimes.

## Product Behavior

1. Registered runtime IDs are defined once in the runtime domain.
2. The active session runtime filter uses the registered runtime ID source instead of duplicating string literals.
3. `cursor` is not advertised as a registered runtime until an adapter is actually registered.
4. `clientForRuntimeId()` rejects unknown runtime IDs instead of returning Claude.
5. `runtimeIdForClient()` rejects unknown legacy clients instead of returning Claude.
6. Existing Claude Code and Codex behavior remains unchanged.

## Non-Goals

- Do not remove the persisted legacy `client` column in this tranche.
- Do not migrate every UI and repository type from `AgentClient` to `RuntimeId`.
- Do not add a Cursor adapter.
- Do not change public API response shapes except to avoid silent fallback.

## Acceptance Criteria

1. `RuntimeId` no longer contains a registered `cursor` literal without an adapter.
2. `SessionRuntimeId` derives from the runtime-domain registered ID type.
3. Runtime filter parsing accepts only registered runtime IDs and reports unknown IDs as invalid.
4. Unknown runtime IDs do not map to Claude.
5. Unknown legacy clients do not map to Claude.
6. Regression tests cover known mappings and unknown rejection.
7. Focused tests, `bun run typecheck`, `bun test`, and `bun run build` pass.
