# Web Mode Hook Availability Product Spec

Issue: https://github.com/majiayu000/keepline/issues/101

## Summary

Users need a clear contract between `keepline web`, `keepline daemon`, and
`keepline hooks install` so hook-backed realtime behavior is not silently
unavailable.

## Product Behavior

1. `keepline web` must document whether it starts only the Web UI or also a hook
   receiver.
2. `keepline daemon` must remain the authoritative background path for the hook
   server unless a deliberate web-mode hook server is implemented.
3. When hooks are installed but the hook server is not running, CLI status and
   Web API status must surface an explicit degraded state.
4. Any web-mode hook server must avoid daemon port conflicts.
5. GH79 loopback, Origin, and Host request guards must remain unchanged.

## Non-Goals

- Do not weaken hook server request security.
- Do not auto-install hooks.
- Do not start duplicate hook servers on the same port.

## Acceptance Criteria

1. Status output distinguishes hooks installed from hook receiver running.
2. Web API exposes the same degraded hook availability state.
3. README command descriptions match actual runtime behavior.
4. Tests cover installed-but-not-running and running states.

