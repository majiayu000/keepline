# Completed Session Sync Preservation Product Spec

Issue: https://github.com/majiayu000/keepline/issues/77

## Summary

When a user marks a session completed, later sync cycles must preserve that explicit completed state. File rescans can refresh metadata, but they must not turn a user-completed session back into `lost`, `idle`, `waiting`, or `running`.

## User Problem

Users mark sessions completed to remove them from active recovery/attention workflows. If the next JSONL sync recomputes status from process state and overwrites `completed`, the dashboard shows completed work as lost or idle again.

## Product Behavior

1. A persisted `completed` session remains `completed` during later syncs.
2. `completedAt` is not overwritten by scanner metadata.
3. Sync may still refresh non-authoritative metadata such as title, message counts, tool info, and usage.
4. Non-completed sessions keep existing behavior: no process still becomes `lost`; live process can become `running`, `waiting`, or `idle`.

## Non-Goals

- Do not infer completed from tools or transcript text.
- Do not change the explicit `completeSession` API behavior.
- Do not change retention cleanup semantics.

## Acceptance Criteria

1. Existing completed session plus no process plus scanned JSONL remains completed after sync.
2. `completedAt` remains the original explicit completion timestamp.
3. Existing non-completed session can still become lost when no process exists.
4. Focused sync regression test covers the completed preservation path.
5. Verification commands pass: focused sync test, `bun run typecheck`, `bun test`, `bun run build`.
