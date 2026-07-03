# Centralized Web Port And Environment Config Product Spec

Issue: https://github.com/majiayu000/keepline/issues/105

## Summary

Keepline config should make web ports, environment variables, and deprecated
fields explicit so runtime behavior is predictable and documented.

## Product Behavior

1. The default Web port must come from one config source while preserving CLI
   `--port` override.
2. Direct environment reads must be centralized or documented as boundary
   exceptions.
3. Unused fields such as `autoDaemon` and stale `embeddingDimension` semantics
   must be removed, wired, or documented.
4. Vite proxy defaults, README examples, and tests must stay aligned.

## Non-Goals

- Do not remove documented security env vars.
- Do not change default bind host.
- Do not change hook port semantics.

## Acceptance Criteria

1. `3377` has a single runtime default source for CLI/API server startup.
2. `keepline web --port` still overrides the default.
3. Config tests cover default port and override behavior.
4. Environment boundary exceptions are documented near config code or docs.
5. Dead config fields are removed, wired, or explicitly documented.

