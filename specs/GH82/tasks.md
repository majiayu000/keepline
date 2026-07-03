# P2 Audit Rollup Triage And Hardening Task Plan

Issue: https://github.com/majiayu000/keepline/issues/82

Product spec: `specs/GH82/product.md`
Tech spec: `specs/GH82/tech.md`

## Tasks

### SP82-T1: Add SpecRail packet and scope map

Done when:

- `specs/GH82/product.md` exists.
- `specs/GH82/tech.md` exists.
- `specs/GH82/tasks.md` exists.
- Scope explicitly avoids duplicating open P0/P1 PRs.

Verify:

```sh
test -f specs/GH82/product.md
test -f specs/GH82/tech.md
test -f specs/GH82/tasks.md
```

### SP82-T2: Parse Codex usage events

Done when:

- Codex parser accumulates supported `event_msg` usage records.
- Missing or malformed usage records are ignored without breaking session parse.
- Parsed Codex sessions include non-zero `usageStats` when usage telemetry is
  present.

Verify:

```sh
bun test src/__tests__/codex.parser.test.ts
```

### SP82-T3: Remove silent/stale pricing behavior

Done when:

- LiteLLM pricing import retains non-Anthropic model entries with token costs.
- Unknown-model fallback emits a warning once per model.
- Usage extraction no longer caches per-model pricing forever.

Verify:

```sh
bun test src/__tests__/usage.pricing.test.ts src/__tests__/usage.extractor.test.ts
```

### SP82-T4: Harden daemon and SQLite startup

Done when:

- Daemon scheduler initializes pricing before scans.
- Fatal daemon errors log, emit, stop, and exit with code 1.
- SQLite sets a non-zero busy timeout.

Verify:

```sh
bun run typecheck
```

### SP82-T5: Validate session API pagination

Done when:

- Invalid `limit` and `offset` return HTTP 400.
- Valid pagination behavior remains unchanged.

Verify:

```sh
bun test src/__tests__/sessions.route-basic.test.ts
```

### SP82-T6: Prevent PID reuse continuity matches

Done when:

- Existing PID continuity still works for compatible process start times.
- Reused PID with an incompatible start time falls back to matching logic and
  does not attach to stale session history.

Verify:

```sh
bun test src/__tests__/session.process-matcher.test.ts
```

### SP82-T7: Clear stale nullable session fields

Done when:

- Upsert preserves nullable fields when omitted.
- Upsert clears nullable fields when the key is present with `undefined` or
  `null`.

Verify:

```sh
bun test src/__tests__/session-repository.test.ts
```

### SP82-T8: Reset optional events table

Done when:

- `resetDatabase()` drops `events`.
- A test proves rows inserted through EventStore do not survive reset.

Verify:

```sh
bun test src/__tests__/reset-database.test.ts
```

### SP82-T9: Full verification and PR gate

Done when:

- Focused tests pass.
- `bun run typecheck` passes.
- `bun test` passes.
- `bun run build` passes.
- PR uses `Refs #82`, not `Closes #82`.

### SP82-T10: Close remaining deterministic hardening gaps

Done when:

- SQLite opens the database through the runtime `getKeeplineDb()` getter.
- Process scanning treats no supported Claude/Codex process as an empty result.
- TypeScript deprecation handling keeps `bun run typecheck` working under the
  installed TypeScript range.

Verify:

```sh
bun test src/__tests__/reset-database.test.ts src/__tests__/process-parser.test.ts src/__tests__/sessions.route-basic.test.ts
bun run typecheck
```
