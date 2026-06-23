# Project Workspaces Tech Spec

Product spec: `specs/project-workspaces/PRODUCT.md`

## Context

- `README.md:37` - Keepline monitors local agent runtime sessions in real time, with Codex and Claude Code as supported runtime adapters.
- `README.md:57` - The README already promises project overview as "Aggregated by project."
- `docs/PROJECTS_VIEW_DESIGN.md:5` - The existing design introduces a Projects tab grouped by project/directory.
- `docs/PROJECTS_VIEW_DESIGN.md:69` - The current project model is path, name, sessions, stats, currentTask, and lastActiveAt.
- `src/domain/session/entity.ts:18` - Session data already carries the owning agent `client`.
- `src/domain/session/entity.ts:21` - Session data has a working directory.
- `src/web/client/src/types/project.ts:18` - Frontend `ProjectInfo` currently treats `path` as the full directory path.
- `src/web/client/src/hooks/useProjects.ts:43` - The hook comment says it aggregates by project name.
- `src/web/client/src/hooks/useProjects.ts:52` - The implementation groups by `extractProjectName(directory)`, merging same-basename paths.
- `src/web/client/src/App.tsx:67` - Projects aggregation intentionally uses all sessions, not the filtered session list.
- `src/web/client/src/App.tsx:131` - Project click currently switches to Sessions and writes the path into text search.
- `src/web/api/routes/sessions.ts:62` - `/api/sessions` owns list, pagination, and basic/full response modes.
- `src/web/api/routes/sessions.ts:88` - `/api/sessions` currently filters by status.
- `src/web/api/routes/sessions.ts:93` - `/api/sessions` already supports a `client=claude|codex` filter.
- `src/services/session.aggregator.ts:83` - Existing aggregation supports exact client filtering.
- `src/services/session.aggregator.ts:87` - Existing directory filtering uses substring matching.
- `src/services/session.aggregator.ts:153` - Backend can already group sessions by exact directory.
- `src/__tests__/projects.test.ts:368` - Project aggregation tests already exist and can be extended.
- `/Users/lifcc/Desktop/code/AI/tool/stash/shared/src/project.ts:1` - stash derives projects from paths in MVP.
- `/Users/lifcc/Desktop/code/AI/tool/stash/shared/src/agent-session.ts:1` - stash normalizes client-aware agent sessions.
- `/Users/lifcc/Desktop/code/AI/tool/stash/client/src/api/workboard.ts:4` - stash exposes project summaries with sessions.
- `/Users/lifcc/Desktop/code/AI/tool/stash/client/src/workbench/concepts/ConceptA.tsx:12` - stash's card wall is the closest UI reference for project-first browsing.
- `/Users/lifcc/Desktop/code/AI/tool/stash/client/src/workbench/concepts/ConceptK.tsx:39` - stash's project workbench is a future reference, not MVP scope.

The repo has a partial Projects view, but it is not yet a durable project workspace. The most important current bug is identity drift: grouping by basename conflicts with the design document's directory grouping and will misclassify same-name repos or worktrees. The second gap is project click behavior: using text search as the filter is broad and can match unrelated session fields.

## Proposed Changes

1. Add a backend project identity helper.

   Suggested module: `src/services/project.identity.ts`.

   ```ts
   export interface ProjectIdentity {
     id: string;
     rootPath: string;
     name: string;
     displayPath: string;
     source: 'git-root' | 'cwd' | 'unknown';
   }
   ```

   Resolution rules:

   - Normalize path separators and trim trailing slashes.
   - Expand `~` when present.
   - If the path exists, walk upward until `.git` is found and use that directory as `rootPath`.
   - Treat `.git` files and `.git` directories as git roots so worktrees work.
   - If no git root is found, use the normalized cwd.
   - If the cwd path no longer exists, still use the normalized cwd as the project root so historical or deleted worktrees remain filterable.
   - Return the stable `unknown` identity only when the cwd string is empty or cannot be normalized at all.
   - Cache resolution per directory during a request to avoid repeated filesystem walks.

2. Use stable collision-resistant project IDs.

   Follow stash's local-first approach, but avoid merging by basename or by lossy path slugs:

   ```ts
   import { createHash } from 'node:crypto';

   export function projectIdFromPath(path: string): string {
     const normalized = normalizeProjectPath(path);
     return normalized
       ? `path-${createHash('sha256').update(normalized).digest('hex').slice(0, 16)}`
       : 'unknown';
   }
   ```

   Keep the readable path in `rootPath` and `displayPath`; do not use a lowercased separator-replaced slug as the grouping key. Slugs such as `/tmp/a-b` and `/tmp/a/b` can collide, and case-sensitive filesystems can distinguish paths that a lowercased slug would merge.

3. Add a project aggregation service.

   Suggested module: `src/services/project.aggregator.ts`.

   ```ts
   export interface ProjectSummary {
     id: string;
     rootPath: string;
     name: string;
     displayPath: string;
     sessions: AggregatedSession[];
     stats: ReturnType<typeof getSessionStats>;
     clientCounts: Partial<Record<AgentClient | 'unknown', number>>;
     currentTask?: string;
     lastActiveAt: Date;
     totalUsage?: unknown;
   }
   ```

   Build summaries from `getAggregatedSessions()` or `getAggregatedSessionsBasic()` depending on the route's fields mode. Group by `ProjectIdentity.id`, not project name. Preserve the full root path for exact filters.

4. Extend the sessions API with exact project filtering.

   Keep `directory` substring filtering for backward compatibility and CLI behavior, but add an exact project filter:

   - `GET /api/sessions?projectRoot=<absolute path>`
   - Optional later alias: `GET /api/sessions?projectId=<path-hash-id>`

   Filtering order:

   1. Load aggregated sessions.
   2. Resolve project identity for each session.
   3. Apply exact `projectRoot` or `projectId`.
   4. Apply status/client/search filters.
   5. Sort and paginate.

5. Add a projects API.

   Suggested route: `src/web/api/routes/projects.ts`.

   - `GET /api/projects?fields=basic|full`
   - Returns `{ success, data: { projects, stats } }`.
   - `fields=basic` should avoid expensive usage hydration, matching `/api/sessions?fields=basic`.
   - Auth middleware should match the sessions route.

   This keeps the client from reimplementing backend project identity and lets CLI or future Tauri surfaces reuse the same contract.

6. Update frontend project types and hook behavior.

   - Add `id`, `rootPath`, `displayPath`, and `clientCounts` to `ProjectInfo`.
   - Change `aggregateProjects()` so tests and fallback client aggregation group by root path.
   - Keep `useProjects(sessions)` only as a test helper or as a fallback when it has the full unpaginated session set.
   - Do not aggregate the Projects overview from the current paginated Sessions list. If `GET /api/projects` is unavailable, fetch all session pages for the fallback or show a degraded Projects loading/error state instead of hiding projects past the first page.
   - Preserve `src/web/client/src/App.tsx:67` behavior: Projects aggregation must come from unfiltered sessions.

7. Replace text-search project selection with explicit state.

   In `DashboardApp`, introduce:

   ```ts
   const [selectedProjectRoot, setSelectedProjectRoot] = useState<string | null>(null);
   ```

   The Sessions tab should receive sessions filtered by selected project root plus the existing search/status filters. Search text should not be overwritten when a project is selected. Clearing the project filter restores the existing search.

8. Use existing client metadata.

   Aggregate `client` counts from session rows. Use a default of `unknown` only when reading older or degraded data that predates the `client` field. Do not infer Codex from title text, tab labels, or shell process names when a parsed client field is available.

9. Extend realtime change detection for project activity.

   `src/web/api/server.ts:127` currently hashes aggregate stats plus `sessionId:status`. Project Workspaces needs activity changes too, so the realtime state hash should include the fields that drive project cards:

   - `sessionId`
   - `client`
   - `status`
   - `directory` or resolved `projectId`
   - `lastActiveAt`
   - `title` or the selected current-task source
   - usage totals when card totals depend on them

   Alternatively, include the serialized `ProjectSummary` payload in the hash once `/api/projects` is the card source of truth. A session that only updates `lastActiveAt` or task text must still broadcast enough data for the owning project card to refresh.

10. Keep project metadata derived in MVP.

   No database migration is required for the first pass. Persisted project aliases, icons, pinned projects, and notes should be follow-ups. This matches stash's current "projects are derived from paths in MVP" strategy.

## Rejected Alternatives

- Use Warp tab names as the project source. They are presentation state and can drift from cwd.
- Keep using text search as project filtering. It is convenient, but it is not exact and can match prompts, files, and unrelated paths.
- Group by basename. This is the current client behavior and it breaks same-name repos and worktrees.
- Port stash's todo, project knowledge, skills, or decision domains into Keepline. They are useful references, but they are outside the monitoring MVP.

## Testing and Validation

| Product Behavior | Verification |
|---|---|
| 1, 2 | Unit tests for `resolveProjectIdentity()` with git root, cwd fallback, same basename paths, worktree `.git` file, missing cwd, and unknown cwd. |
| 3, 4 | Project aggregation tests covering client counts, status counts, current task, lastActiveAt sort, and usage aggregation. |
| 5, 6 | Route tests for `/api/sessions?projectRoot=...` proving exact filtering composes with status, client, and pagination. |
| 7 | Client hook test or component test proving Projects overview still uses all sessions while Sessions search is active. |
| 8, 9 | WebSocket or notification unit test proving hidden sessions still trigger changes and include project context. |
| 10 | Route or service test proving unreadable paths degrade to cwd or unknown instead of throwing. |
| 11 | Manual keyboard check on Projects tab: tab to card, Enter selects project, clear filter returns to all sessions. |
| 12 | Visual/manual check on narrow desktop width that long paths elide without hiding the project name. |

Commands:

- `bun test src/__tests__/projects.test.ts`
- `bun test src/__tests__/sessions.route-basic.test.ts`
- `bun test`
- `bun run typecheck`
- `bun run build`

## Rollout Plan

1. Land project identity and aggregation tests first.
2. Add exact project filtering to `/api/sessions`.
3. Add `/api/projects` and wire Projects view to it.
4. Update the existing Projects click behavior to set project filter state instead of search text.
5. Add client badges/counts from existing session metadata.

## Risks

- Filesystem walks can be expensive if done per render. Mitigation: resolve on the backend with request-scoped or process-scoped caching.
- Some historical session directories may no longer exist. Mitigation: fall back to normalized cwd and never drop those sessions.
- Path-derived IDs expose local path shape in local URLs. Mitigation: acceptable for local-only MVP; switch to hashed IDs before any remote sharing.
- Older persisted rows may lack client metadata. Mitigation: aggregate `unknown` client counts and gate visible badges on available metadata.

## Follow-Ups

- Persist custom project aliases, icons, pinned order, and hidden/archive state.
- Add a project detail page inspired by stash Concept K, but limited to sessions, usage, files touched, and recovery actions.
- Add CLI support such as `keepline projects` and `keepline list --project <root>`.
- Consider optional Warp tab title helpers only after project identity is stable inside Keepline.
