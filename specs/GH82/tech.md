# P2 Audit Rollup Triage And Hardening Tech Spec

Product spec: `specs/GH82/product.md`

Issue: https://github.com/majiayu000/keepline/issues/82

## Implementation Scope

This tranche changes only files needed for non-overlapping hardening:

- `src/adapters/codex/parser.ts`
- `src/services/usage.pricing.ts`
- `src/services/usage.extractor.ts`
- `src/services/daemon.scheduler.ts`
- `src/infrastructure/database/sqlite.ts`
- `src/infrastructure/database/repositories/session.repository.ts`
- `src/db/migrations.ts`
- `src/web/api/routes/sessions.ts`
- `src/services/session.process-matcher.ts`
- focused tests for those modules

LanceDB predicate handling, hook server installation semantics, retention
scheduling, runtime identity vocabulary, and basic usage serialization are
covered by existing open PRs and should not be duplicated here.

## Codex Usage Parsing

Codex JSONL may include usage telemetry in `event_msg` payloads. The parser
must tolerate multiple telemetry shapes because historical Codex event payloads
are not a stable public API.

Supported shapes:

```ts
{
  type: 'event_msg',
  timestamp: string,
  payload: {
    model?: string,
    usage?: {
      input_tokens?: number,
      output_tokens?: number,
      cache_creation_input_tokens?: number,
      cache_read_input_tokens?: number
    }
  }
}
```

```ts
{
  type: 'event_msg',
  payload: {
    msg?: {
      model?: string,
      usage?: {
        input_tokens?: number,
        output_tokens?: number
      }
    }
  }
}
```

The parser should also accept camelCase token fields for compatibility with
normalized Codex event emitters. Usage with missing numeric input or output
tokens is ignored.

Use the existing usage accumulator helpers so cost logic stays shared with
Claude usage aggregation.

## Pricing

`fetchLiteLLMPricing()` should retain every LiteLLM model entry with both
`input_cost_per_token` and `output_cost_per_token`, regardless of provider. It
should keep provider-prefixed keys such as `openai/gpt-4o-mini` and exact keys
as LiteLLM publishes them.

Unknown-model fallback remains available for continuity, but `getModelPricing`
must emit a warning the first time a model falls back so silent mispricing does
not pass as precise output.

The usage extractor should not keep an independent forever cache of per-token
pricing. Recomputing the per-token values from the current pricing config is
cheap and prevents stale cost models after pricing refresh.

## Daemon Fatal Errors

`startScheduler()` should call `initPricing()` after database migrations and
before initial sync. Failure to fetch remote pricing still falls back to default
pricing inside the pricing module.

`runDaemon()` should still log and emit errors for `uncaughtException` and
`unhandledRejection`, then stop the scheduler and exit with code 1. Continuing
after these fatal errors is unsafe because the daemon may be partially broken
while status still appears running.

## SQLite

After opening the database, set:

```sql
PRAGMA busy_timeout = 5000
```

Keep WAL and foreign keys enabled.

## Sessions API Pagination

Parse `limit` and `offset` through a small validator:

- default `limit`: 50
- max `limit`: 100
- default `offset`: 0
- invalid, non-integer, negative, or zero-limit values return HTTP 400

## PID Continuity

PID continuity remains preferred only when the directory/client group matches
and the process start time is compatible with the stored session timeline.

A process is compatible when its start time is not materially after the stored
session's last active timestamp. A small clock-skew tolerance is allowed.
Incompatible PID reuse falls back to the normal assignment algorithm instead of
being accepted as continuity.

## Nullable Session Fields

Repository upsert already uses presence checks for `pid` and `tty`. Apply the
same pattern to nullable activity fields that can become stale:

- `lastTool`
- `lastToolInput`
- `currentFile`
- `lastMessage`
- `completedAt`
- `agentId`
- `parentSessionId`
- `usageStats`
- `toolCalls`

When a field is omitted, preserve the old value. When a field key is present
with `undefined` or `null`, write NULL.

## Reset Database

`resetDatabase()` must drop `events` before re-running migrations so tests and
local resets do not retain optional EventStore rows.
