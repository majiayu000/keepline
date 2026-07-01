# Project Workspaces Task Plan

Product spec: `specs/project-workspaces/PRODUCT.md`
Tech spec: `specs/project-workspaces/TECH.md`

## Thread Lane Map

- `PW-L1`: Coordinator/spec owner, read-only after this plan lands. Owns scope, route order, and handoff notes.
- `PW-L2`: Backend worker. Owns project identity, aggregation, sessions/projects APIs, and backend tests.
- `PW-L3`: Frontend worker. Owns Projects view data flow, explicit project filter state, client badges, and accessibility checks.
- `PW-L4`: Realtime/notification worker. Owns WebSocket change detection and project-context notification behavior.
- `PW-L5`: Reviewer, read-only. Owns spec-vs-implementation review and verification risk notes.

No two writable lanes may edit the same file in the same tranche. `AGENTS.md`, `CLAUDE.md`, `.claude/*`, hooks, settings, global config, and secret-bearing files are forbidden unless a maintainer explicitly asks.

## Tasks

### SPPW-T1: Confirm SpecRail artifacts and scope

Owner: coordinator

Dependencies: existing Project Workspaces product and tech specs

Done when:

- `specs/project-workspaces/PRODUCT.md` exists.
- `specs/project-workspaces/TECH.md` exists.
- `specs/project-workspaces/tasks.md` exists.
- MVP scope remains derived project identity, exact project filtering, client counts, Projects overview, and realtime project-card freshness.
- Persisted aliases, icons, pinned order, project notes, Warp tab inspection, and direct agent spawning remain follow-ups.

Verify:

```sh
test -f specs/project-workspaces/PRODUCT.md
test -f specs/project-workspaces/TECH.md
test -f specs/project-workspaces/tasks.md
```

### SPPW-T2: Add project identity helper

Owner: backend worker

Dependencies: SPPW-T1

Expected writable files:

- `src/services/project.identity.ts`
- project identity unit tests

Done when:

- `resolveProjectIdentity()` normalizes paths, expands `~`, trims trailing separators, and returns `ProjectIdentity`.
- Existing paths inside git repos resolve to repo/worktree roots, including `.git` files and `.git` directories.
- Missing historical cwd values remain normalized cwd roots instead of disappearing.
- Empty or unnormalizable cwd values map to stable `unknown`.
- Project IDs are collision-resistant path hashes, not lowercased basename or lossy slugs.
- Request-level caching avoids repeated filesystem walks for the same directory.

Verify:

```sh
bun test src/__tests__/project-identity.test.ts
bun run typecheck
```

### SPPW-T3: Add project aggregation service

Owner: backend worker

Dependencies: SPPW-T2

Expected writable files:

- `src/services/project.aggregator.ts`
- `src/__tests__/projects.test.ts`

Done when:

- Project summaries group by `ProjectIdentity.id`, not project name.
- Each summary includes `id`, `rootPath`, `name`, `displayPath`, sessions, stats, client counts, current task, last activity, and usage totals when data exists.
- Same-basename repos and worktrees stay separate.
- Older sessions without `client` metadata count under `unknown` instead of being inferred from title or shell state.
- Aggregation can run from the full unfiltered session set.

Verify:

```sh
bun test src/__tests__/projects.test.ts
bun run typecheck
```

### SPPW-T4: Add exact project filtering to sessions API

Owner: backend worker

Dependencies: SPPW-T2

Expected writable files:

- `src/services/session.aggregator.ts`
- `src/web/api/routes/sessions.ts`
- sessions route tests

Done when:

- `GET /api/sessions?projectRoot=<absolute path>` filters by exact resolved project root.
- Existing `directory` substring filtering remains available for backward compatibility.
- Filtering order is load sessions, resolve project identity, apply project filter, apply status/client/search filters, sort, then paginate.
- `projectRoot` composes with status, client, search, sorting, and pagination.
- Invalid or unsafe project filter inputs return explicit errors instead of broad fallback behavior.

Verify:

```sh
bun test src/__tests__/sessions.route-basic.test.ts
bun test src/__tests__/projects.test.ts
bun run typecheck
```

### SPPW-T5: Add projects API

Owner: backend worker

Dependencies: SPPW-T3

Expected writable files:

- `src/web/api/routes/projects.ts`
- `src/web/api/routes/index.ts`
- `src/web/api/server.ts`
- projects route tests

Done when:

- `GET /api/projects?fields=basic|full` returns `{ success, data: { projects, stats } }`.
- The route uses the same auth middleware pattern as sessions.
- Basic mode avoids unnecessary expensive hydration.
- Empty, loading, and degraded states are explicit in the API response or logs.
- A parser failure for one client/runtime does not hide sessions from another client/runtime.

Verify:

```sh
bun test src/__tests__/projects.route.test.ts
bun run typecheck
```

### SPPW-T6: Wire Projects view to stable project summaries

Owner: frontend worker

Dependencies: SPPW-T3, SPPW-T5

Expected writable files:

- `src/web/client/src/types/project.ts`
- `src/web/client/src/hooks/useProjects.ts`
- `src/web/client/src/components/**`
- project hook/component tests

Done when:

- Frontend `ProjectInfo` includes `id`, `rootPath`, `displayPath`, and `clientCounts`.
- Projects overview uses `/api/projects` or a proven full-session fallback, not the current paginated Sessions list.
- Project cards show status counts, client badges/counts, latest task, last activity, total sessions, and usage totals when available.
- Long absolute paths are visually compact while preserving full path access in detail or tooltip surfaces.
- Empty/loading/error states are explicit and do not produce a blank Projects tab.

Verify:

```sh
cd src/web/client && bun run typecheck && bun run build
bun test src/__tests__/projects.test.ts
```

### SPPW-T7: Replace text-search project selection with explicit filter state

Owner: frontend worker

Dependencies: SPPW-T4, SPPW-T6

Expected writable files:

- `src/web/client/src/App.tsx`
- `src/web/client/src/components/**`
- related frontend tests

Done when:

- Selecting a project sets `selectedProjectRoot` or equivalent explicit state.
- Sessions view filters by the selected project root plus existing search/status/client filters.
- Selecting a project does not overwrite search text.
- Clearing the project filter restores the previous global search/status state.
- Keyboard users can tab to a project card, press Enter to select it, and clear the filter predictably.

Verify:

```sh
cd src/web/client && bun run typecheck && bun run build
```

### SPPW-T8: Update realtime project-card and notification behavior

Owner: realtime/notification worker

Dependencies: SPPW-T3, SPPW-T5

Expected writable files:

- `src/web/api/server.ts`
- notification service files
- realtime/notification tests

Done when:

- WebSocket change detection includes project-card inputs such as session id, client, status, directory or project ID, last activity, current task, and usage totals when used.
- A session that only updates `lastActiveAt` or visible task text still refreshes the owning project card.
- Notifications remain session-based but include project context when available.
- Notification delivery does not depend on the current selected project filter.

Verify:

```sh
bun test src/__tests__/projects.realtime.test.ts
bun test src/__tests__/notifications.test.ts
bun run typecheck
```

### SPPW-T9: Manual UI and accessibility verification

Owner: frontend worker

Dependencies: SPPW-T6, SPPW-T7, SPPW-T8

Done when:

- Projects tab is keyboard reachable.
- Focus moves predictably after selecting a project and clearing the filter.
- Same-basename projects are visually distinguishable.
- Long paths elide without hiding the project name or overlapping neighboring UI.
- Narrow desktop and standard desktop widths show coherent card layout.

Verify:

```sh
cd src/web/client && bun run dev
```

Manual checks:

- Tab to a project card and press Enter.
- Confirm Sessions shows only that exact project root.
- Confirm search text remains unchanged.
- Clear project filter and confirm the previous search/status/client filters remain.

### SPPW-T10: Full verification and handoff

Owner: verification owner

Dependencies: SPPW-T2, SPPW-T3, SPPW-T4, SPPW-T5, SPPW-T6, SPPW-T7, SPPW-T8, SPPW-T9

Done when:

- Targeted backend and frontend tests pass.
- Full local typecheck, test, build, and web client build pass.
- `git diff --check` passes.
- If `checks/check_workflow.py` remains absent, the PR notes that path/link review plus `git diff --check` were used as the local substitute.
- Human review and merge gates remain intact.

Verify:

```sh
bun test src/__tests__/projects.test.ts
bun test src/__tests__/sessions.route-basic.test.ts
bun test
bun run typecheck
bun run build
cd src/web/client && bun run typecheck && bun run build
git diff --check
```

## Handoff Notes

- This plan does not authorize implementation, PR creation, merge, force push, release, or remote issue mutation by itself.
- There is no linked GitHub issue recorded in the existing Project Workspaces specs. If this work becomes remote-tracked, create or request a linked issue before opening an implementation PR.
- `checks/route_gate.py` and `checks/check_workflow.py` are not present in this repository at the time this plan was written; use the explicit verification commands above unless those checks are added later.
