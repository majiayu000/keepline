# P2 Audit Rollup Triage And Hardening Product Spec

Issue: https://github.com/majiayu000/keepline/issues/82

## Summary

GH82 is a P2 technical-debt rollup from codebase audit findings. It mixes
immediate correctness bugs, security-adjacent persistence risks, stale findings,
items already covered by open P1 PRs, and larger architecture cleanups.

This tranche converts the rollup into SpecRail-owned work and fixes the
high-confidence items that can be changed independently of the current P0/P1 PR
queue. It does not close GH82 until every checklist item is either fixed,
verified stale, or split into accepted follow-up issues.

## User Problem

Users trust Keepline for cost visibility, session recovery, and live status. A
few silent or stale behaviors erode that trust:

1. Codex sessions can display zero cost even when token usage exists in the
   session stream.
2. Non-Anthropic model pricing can be omitted or silently priced as Claude.
3. The daemon can run with stale pricing or keep running after fatal process
   errors.
4. Concurrent SQLite writers can fail with avoidable `SQLITE_BUSY` errors.
5. API pagination with invalid numbers can return misleading empty results.
6. PID reuse can make an old session look attached to a new process.
7. Session fields that should clear can remain stale forever.

## Product Behavior

1. Codex token and cost data should be parsed from supported Codex JSONL usage
   events and persisted into `usageStats` when present.
2. Pricing should use LiteLLM entries for all providers when token costs are
   available, while preserving known Claude defaults.
3. Unknown model pricing should be visible in logs instead of silently looking
   precise.
4. The daemon should initialize pricing before sync cycles and exit on
   uncaught exceptions or unhandled rejections so a supervisor can restart it.
5. SQLite should wait briefly for concurrent writers instead of failing
   immediately.
6. Session list pagination should reject invalid `limit` or `offset` values
   with a 400 response.
7. PID continuity should only preserve a PID match when process start time is
   compatible with the previous session.
8. Explicit nullable session fields should be clearable when an updater sends
   the field with `undefined` or `null`.
9. Test database reset should remove the optional `events` table as well as the
   core migrated tables.

## Non-Goals

- Do not duplicate the open GH74 LanceDB predicate hardening PR.
- Do not duplicate the open GH75 hook pipeline PR.
- Do not duplicate the open GH78-GH81 P1 fixes.
- Do not refactor frontend runtime validation, status displays, env config, or
  adapter layer boundaries in this tranche.
- Do not remove public EventStore exports in a hardening tranche.
- Do not close GH82 from this partial tranche.

## Acceptance Criteria

1. `specs/GH82/product.md`, `tech.md`, and `tasks.md` exist.
2. Codex parser tests prove supported `event_msg` usage entries produce
   non-zero `usageStats`.
3. Pricing tests prove OpenAI/LiteLLM-style non-Anthropic entries are retained
   and unknown model fallback is logged.
4. Daemon startup initializes pricing and fatal process errors exit instead of
   continuing silently.
5. SQLite connection setup applies a non-zero busy timeout.
6. Session route tests prove invalid `limit` and `offset` return HTTP 400.
7. Process matcher tests prove reused PIDs with incompatible start times are not
   treated as continuity.
8. Repository tests prove nullable activity fields can be explicitly cleared.
9. Reset database tests prove the optional `events` table is dropped.
10. Verification passes: focused tests, `bun run typecheck`, `bun test`, and
    `bun run build`.

## Related Work

- PR #84 covers GH74 LanceDB predicate hardening.
- PR #87 covers GH75 hook pipeline restoration.
- PR #90 covers GH78 usage data in basic session payloads.
- PR #91 covers GH79 hook server request guards.
- PR #92 covers GH80 runtime identity contract cleanup.
- PR #93 covers GH81 retention cleanup scheduling.
