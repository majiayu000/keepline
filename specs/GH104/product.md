# Adapter Boundary And Architecture Docs Product Spec

Issue: https://github.com/majiayu000/keepline/issues/104

## Summary

Keepline's adapter layer should not depend on services in ways that create
cross-layer cycles, and architecture docs should match the actual source tree.

## Product Behavior

1. `adapters -> services` imports must be audited and either removed or
   justified.
2. Runtime adapter metadata should not depend on high-level service modules.
3. `AGENTS.md` and `CLAUDE.md` must describe the source tree that exists today.
4. A static test must catch future unwanted adapter-to-service imports.

## Non-Goals

- Do not perform a full DDD rewrite.
- Do not move unrelated business logic.
- Do not change public CLI behavior.

## Acceptance Criteria

1. All current adapter-to-service imports are listed and resolved or documented.
2. Architecture docs no longer mention a non-existent `src/application/` layer.
3. Static architecture tests enforce the chosen boundary.
4. Typecheck and focused tests pass.

