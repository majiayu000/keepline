# Runtime-Neutral Agent Orchestration Task Plan

Issue: https://github.com/majiayu000/claude-hub/issues/44

Product spec: `specs/GH44/product.md`
Tech spec: `specs/GH44/tech.md`
Discovery follow-up: `specs/GH44/cursor-runtime-discovery.md`

## Thread Lane Map

- `GH44-L1`: Coordinator/spec owner, read-only after this plan lands. Owns scope, dependency order, and PR handoff.
- `GH44-L2`: Runtime foundation worker. Owns runtime domain contracts, registry, Claude Code wrapper, Codex adapter, and adapter tests.
- `GH44-L3`: Work item and evidence worker. Owns WorkItem persistence/API, WorkItemSessionLink, ProgressEvidence, and Workboard projections.
- `GH44-L4`: UI/API integration worker. Owns sessions/project API extensions, runtime badges/filters, docs/UI wording, and browser verification.
- `GH44-L5`: Reviewer, read-only. Owns spec-vs-implementation review and verification risk notes.

No two writable lanes may edit the same file in the same tranche. `AGENTS.md`, `CLAUDE.md`, `.claude/*`, hooks, settings, global config, and secret-bearing files are forbidden unless a maintainer explicitly asks.

## Tasks

### SP44-T1: Confirm SpecRail artifacts and scope

Owner: coordinator

Dependencies: #44 issue body and existing GH44 docs

Done when:

- `specs/GH44/product.md` exists.
- `specs/GH44/tech.md` exists.
- `specs/GH44/tasks.md` exists.
- The scope keeps Codex, Claude Code, and future Cursor as runtime adapters, not product boundaries.
- Cursor remains discovery-only until a stable source contract is selected.

Verify:

```sh
test -f specs/GH44/product.md
test -f specs/GH44/tech.md
test -f specs/GH44/tasks.md
test -f specs/GH44/cursor-runtime-discovery.md
```

### SP44-T2: Add WorkItem foundation

Owner: Work item and evidence worker

Dependencies: SP44-T1

Expected writable files:

- `src/domain/work-item/**`
- `src/infrastructure/database/**`
- `src/web/api/routes/work-items.ts`
- focused work-item tests

Done when:

- Durable WorkItem storage supports inbox, planned, active, blocked, done, archived, and archived visibility rules.
- Inbox/Todo/Idea capture works without selecting a project.
- Formal status transitions require `statusSource` of `user` or `accepted_agent_suggestion`.
- Agent suggestions cannot silently change planned/done/blocked state.

Verify:

```sh
bun test src/__tests__/work-items.test.ts
bun run typecheck
```

### SP44-T3: Add runtime domain contracts and registry

Owner: runtime foundation worker

Dependencies: SP44-T1

Expected writable files:

- `src/domain/runtime/**`
- `src/adapters/runtimes/registry.ts`
- runtime contract and registry tests

Done when:

- `RuntimeDescriptor`, `RuntimeSession`, `RuntimeCommand`, `RuntimeScanResult`, and `AgentRuntimeAdapter` are defined.
- `RuntimeRegistry` registers adapters by stable `runtimeId` and rejects duplicates.
- `scanAll()` isolates adapter failures and returns structured per-runtime errors.
- Adapters declaring `resume`, `quota`, `plans`, or `hooks` have matching command builders, providers, or explicit compatibility routes.

Verify:

```sh
bun test src/__tests__/runtime.registry.test.ts
bun run typecheck
```

### SP44-T4: Wrap Claude Code as a runtime adapter

Owner: runtime foundation worker

Dependencies: SP44-T3

Expected writable files:

- `src/adapters/runtimes/claude-code.ts`
- Claude parser/scanner adapter tests

Done when:

- Existing Claude Code scanner behavior is preserved through `ClaudeCodeRuntimeAdapter`.
- Runtime ID is `claude-code`; legacy persisted client value `claude` normalizes correctly.
- Default source hints include `~/.claude/projects` and `~/.claude-work/projects`.
- `KEEPLINE_PROJECT_ROOTS` remains a colon-separated absolute-path override.
- Resume, continue, and new recovery modes return structured `RuntimeCommand` values, not shell strings.
- Per-file read/parse failures are exposed as `RuntimeScanResult.errors`.

Verify:

```sh
bun test src/__tests__/claude-runtime-adapter.test.ts
bun test src/__tests__/recovery-security.test.ts
bun run typecheck
```

### SP44-T5: Add Codex runtime adapter

Owner: runtime foundation worker

Dependencies: SP44-T3

Expected writable files:

- `src/adapters/runtimes/codex.ts`
- `src/adapters/Codex/parser/**`
- Codex adapter/parser fixtures and tests

Done when:

- Codex rollout JSONL files under `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` parse into normalized `RuntimeSession` records.
- Missing Codex sessions root is a no-op.
- Bad JSONL files produce structured runtime scan errors without hiding good sessions.
- `turn_context` records are accepted and stored as runtime metadata without replacing `session_meta` identity.
- Codex resume, continue, and new recovery modes return structured `RuntimeCommand` values and use the raw runtime session ID.

Verify:

```sh
bun test src/__tests__/codex.parser.test.ts
bun test src/__tests__/codex-runtime-adapter.test.ts
bun run typecheck
```

### SP44-T6: Integrate runtime sessions into APIs and project filters

Owner: UI/API integration worker

Dependencies: SP44-T3, SP44-T4, SP44-T5

Expected writable files:

- `src/services/session.aggregator.ts`
- `src/web/api/routes/sessions.ts`
- `src/web/api/routes/projects.ts`
- session/project route tests

Done when:

- Sessions expose `runtimeId` while preserving compatibility for existing `client` consumers.
- Runtime filters include legacy `client: "claude"` rows when callers request `runtimeId=claude-code`.
- `projectRoot` filtering resolves cwd to project identity first, then applies exact root matching.
- Project filter composes with search, status, runtime/client, sorting, and pagination.
- Same-basename projects or worktrees do not merge.

Verify:

```sh
bun test src/__tests__/sessions.route-basic.test.ts
bun test src/__tests__/projects.test.ts
bun run typecheck
```

### SP44-T7: Add AgentSession, evidence, and link model

Owner: Work item and evidence worker

Dependencies: SP44-T2, SP44-T3

Expected writable files:

- `src/domain/session/**`
- `src/domain/work-item/**`
- `src/infrastructure/database/repositories/**`
- evidence and link tests

Done when:

- Runtime sessions map to collision-resistant global `AgentSession.id` values.
- `WorkItemSessionLink` supports accepted, pending, and rejected links.
- `ProgressEvidence` requires at least one stable attachment anchor.
- `ProgressEvidence.outcome` drives completion/blocking/failure signals without parsing free text.
- Inferred evidence can suggest progress but cannot mark a WorkItem planned, done, or blocked by itself.

Verify:

```sh
bun test src/__tests__/agent-session-links.test.ts
bun test src/__tests__/progress-evidence.test.ts
bun run typecheck
```

### SP44-T8: Build Workboard projections

Owner: Work item and evidence worker

Dependencies: SP44-T2, SP44-T7

Expected writable files:

- `src/domain/work-item/workboard.ts`
- `src/services/workboard*.ts`
- Workboard projection tests

Done when:

- Now, Waiting, Stale, and Done buckets are projections, not status mutations.
- Bucket precedence follows Done, Waiting, Stale, Now, with archived items hidden by default.
- Waiting sessions do not also appear in Now.
- Stale uses linked session/evidence timestamps and does not fall back to `WorkItem.updatedAt`.
- No evidence renders blank progress instead of guessed completion.
- Suggestions are visibly suggestions until accepted.

Verify:

```sh
bun test src/__tests__/workboard.test.ts
bun run typecheck
```

### SP44-T9: Update runtime-neutral UI, docs, and wording

Owner: UI/API integration worker

Dependencies: SP44-T4, SP44-T5, SP44-T6

Expected writable files:

- `README.md`
- `docs/**`
- `src/web/client/**`
- CLI help and package metadata files, if in scope for the tranche

Done when:

- Dashboard, CLI, API docs, and specs describe Keepline as a runtime-neutral orchestration hub.
- Runtime-specific labels appear only as adapter identity, migration history, compatibility text, or concrete runtime badges.
- Session cards and project cards distinguish Claude Code, Codex, and future runtime identities through metadata, not text guesses.
- No product boundary text says the system is only for Claude Code or only for Codex.

Verify:

```sh
rg -n "Claude Hub|Codex Hub|Claude-only|Codex-only|session monitor" README.md docs src package.json || true
bun run typecheck
cd src/web/client && bun run typecheck && bun run build
```

### SP44-T10: Preserve Cursor as a follow-up discovery track

Owner: coordinator

Dependencies: SP44-T1

Done when:

- `specs/GH44/cursor-runtime-discovery.md` records Cursor source-truth questions and research boundaries.
- No implementation guesses Cursor private storage schemas as adapter truth.
- Any future Cursor implementation is gated on a separate product/tech spec or accepted follow-up issue.

Verify:

```sh
test -f specs/GH44/cursor-runtime-discovery.md
rg -n "Cursor" specs/GH44
```

### SP44-T11: Full verification and handoff

Owner: verification owner

Dependencies: SP44-T2, SP44-T3, SP44-T4, SP44-T5, SP44-T6, SP44-T7, SP44-T8, SP44-T9

Done when:

- Targeted tests for each tranche pass.
- Full local typecheck, test, build, and web client build pass.
- `git diff --check` passes.
- If `checks/check_workflow.py` remains absent, the PR notes that path/link review plus `git diff --check` were used as the local substitute.
- Human review and merge gates remain intact.

Verify:

```sh
bun run typecheck
bun test
bun run build
cd src/web/client && bun run typecheck && bun run build
git diff --check
```

## Handoff Notes

- This plan does not authorize implementation, PR creation, merge, force push, release, or remote issue mutation by itself.
- `checks/route_gate.py` and `checks/check_workflow.py` are not present in this repository at the time this plan was written; use the explicit verification commands above unless those checks are added later.
- Keep PRs narrow: spec/tasks, WorkItem foundation, runtime foundation, Codex adapter, evidence, Workboard, and UI/API wording should be separate tranches unless a maintainer explicitly asks for a combined PR.
