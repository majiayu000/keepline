# Agent Orchestrator Task Plan

Issue: https://github.com/majiayu000/keepline/issues/62

## Thread Lane Map

- `GH62-L1`: Spec planner, read-only. Owns product/tech/task artifact review.
- `GH62-L2`: PR1 worker. Owns deterministic Attention Queue service, CLI overview, API route, route registration, and tests.
- `GH62-L3`: Reviewer, read-only. Owns post-implementation diff review and verification risk check.

No two writable lanes may edit the same file. `AGENTS.md`, `.claude/*`, hooks, settings, and global config are forbidden files unless explicitly requested.

## Tasks

### SP62-T1: Create SpecRail artifacts

Owner: coordinator

Dependencies: #62 issue body

Done when:

- `specs/GH62/product.md` exists.
- `specs/GH62/tech.md` exists.
- `specs/GH62/tasks.md` exists.
- Product scope separates P0 deterministic queue from digest/model/web follow-ups.

Verify:

```sh
test -f specs/GH62/product.md
test -f specs/GH62/tech.md
test -f specs/GH62/tasks.md
```

### SP62-T2: Create child GitHub issues

Owner: coordinator

Dependencies: SP62-T1

Done when:

- Parent #62 is linked from all child issue bodies.
- At least these child issues exist:
  - #63 PR1 deterministic Attention Queue + `keepline overview`
  - #65 Session Digest schema and deterministic digest generator
  - #64 Local model summarizer provider
  - #66 Web Orchestrator view
- Issue-to-PR map is recorded before PR creation.

Verify:

```sh
gh issue list --repo majiayu000/keepline --state open --limit 20
```

### SP62-T3: Implement deterministic Attention Queue service

Owner: PR1 worker

Writable files:

- `src/services/attention.prioritizer.ts`
- `src/services/index.ts`
- `src/__tests__/attention.prioritizer.test.ts`

Done when:

- `buildAttentionOverview()` ranks waiting above lost above high-cost above stale/idle/running.
- Reasons include stable codes, severity, messages, and score contribution.
- Completed sessions are excluded by default.
- No model calls or transcript reads occur.

Verify:

```sh
bun test src/__tests__/attention.prioritizer.test.ts
```

### SP62-T4: Add CLI overview

Owner: PR1 worker

Writable files:

- `src/cli/overview.ts`
- `src/cli/index.ts`

Done when:

- `keepline overview` is registered.
- `--all`, `--limit`, and `--json` are supported.
- CLI initializes migrations and syncs sessions before reading data.

Verify:

```sh
bun run typecheck
```

### SP62-T5: Add Orchestrator overview API

Owner: PR1 worker

Writable files:

- `src/web/api/routes/orchestrator.ts`
- `src/web/api/routes/index.ts`
- `src/web/api/server.ts`
- `src/__tests__/orchestrator.route.test.ts`

Done when:

- `GET /api/orchestrator/overview` returns serialized overview payload.
- Route uses existing auth middleware.
- Invalid numeric query values return 400.
- Old lost sessions are hidden by default with `hiddenOldLost` stats and can be included with `includeOldLost=true`.
- Route tests use relative timestamps for recent and old lost sessions so they remain deterministic over time.

Verify:

```sh
bun test src/__tests__/orchestrator.route.test.ts
```

### SP62-T6: Full local verification

Owner: verification owner

Dependencies: SP62-T3, SP62-T4, SP62-T5

Done when:

- Targeted tests pass.
- Typecheck passes.
- Full test suite passes.
- Build passes.

Verify:

```sh
bun test src/__tests__/attention.prioritizer.test.ts src/__tests__/orchestrator.route.test.ts
bun run typecheck
bun test
bun run build
```

### SP62-T7: Open PR1

Owner: coordinator

Dependencies: SP62-T6

Done when:

- Branch is pushed.
- PR body links parent #62 and PR1 child issue.
- PR body includes SpecRail artifact paths and verification commands.
- PR is not merged without explicit human authorization.

Verify:

```sh
gh pr view --json number,title,state,headRefName,baseRefName,url
```

## Issue-To-PR Map

| Issue | Scope | PR status |
|---|---|---|
| #63 | Deterministic Attention Queue + `keepline overview` | This tranche opens PR1 |
| #65 | Session Digest persistence and deterministic digest generation | Follow-up |
| #64 | Ollama / LM Studio local summarizer provider | Follow-up |
| #66 | Web Orchestrator view using shared API | Follow-up |

## Follow-Up Issues

- #65 Session Digest persistence and deterministic digest generation.
- #64 Ollama / LM Studio local summarizer provider.
- #66 Web Orchestrator view using the same API.
- Optional semantic search/timeline after digest data is stable.
