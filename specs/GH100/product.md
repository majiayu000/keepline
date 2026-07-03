# API And Web Contract Cleanup Product Spec

Issue: https://github.com/majiayu000/keepline/issues/100

## Summary

Keepline's web client should consume only fields the backend actually produces,
and API or WebSocket boundaries should fail predictably when payload shapes
drift.

## Product Behavior

1. Frontend session types must not advertise fields with no backend producer.
2. Orchestrator recommendation actions must match actions that production code
   can emit.
3. `sync:complete` must have an explicit client handler contract.
4. Runtime scan error types must accept the backend's extensible runtime id
   model.
5. Critical API and WebSocket JSON boundaries must use centralized lightweight
   parsing helpers instead of scattered blind assertions.

## Non-Goals

- Do not introduce a heavyweight runtime schema dependency.
- Do not redesign orchestrator ranking.
- Do not remove fields that are actively rendered and produced.

## Acceptance Criteria

1. Dead frontend fields such as `subAgentCount` are removed or backed by a
   producer.
2. The `resume` orchestrator action is either produced by a real path or removed
   from the contract.
3. `sync:complete` updates client state through a named handler path.
4. Runtime scan error frontend types no longer narrow backend runtime ids.
5. Focused tests cover contract helpers and WebSocket update behavior.

