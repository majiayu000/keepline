# Completed Session Sync Preservation Tech Spec

Product spec: specs/GH77/product.md

Issue: https://github.com/majiayu000/keepline/issues/77

## Current Root Cause

`src/services/session.service.ts` computes `detectSessionStatus(process || null, agentSession.lastActiveAt)` for every scanned session. In the existing-session branch, that detected status is passed directly into `repository.upsert`. `detectSessionStatus` never returns `completed`, so a user-completed session is overwritten on the next scan.

The later dead-process loop already excludes completed sessions via `findActiveLightweight()`, but the primary scanned-session update path does not.

## Implementation Plan

- In the existing-session branch, compute:

  ```ts
  const nextStatus = existing.status === 'completed' ? 'completed' : detectedStatus;
  ```

- Use `nextStatus` for `wasLost` and `repository.upsert`.
- Do not pass `completedAt` during sync; repository `COALESCE` keeps the original value.
- Keep metadata refresh behavior unchanged.

## Tests

Add a subprocess-backed sync test that:

1. Uses temporary `HOME` and `KEEPLINE_HOME`.
2. Writes a real Claude project JSONL file inside `.claude/projects`.
3. Inserts a matching DB session with `status: 'completed'`, `completedAt`, and a stale pid.
4. Runs `sessionService.syncSessions({ fullSync: true })`.
5. Asserts status remains `completed`, `completedAt` is unchanged, and `lost` count remains `0`.

This tests the real scanner, process matcher, service, and repository path without weakening production code.
