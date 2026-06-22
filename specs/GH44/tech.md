# Runtime-Neutral Agent Orchestration Tech Spec

Product spec: specs/GH44/product.md

Issue: https://github.com/majiayu000/claude-hub/issues/44

## Context

- AGENTS.md and README.md have historically described the product through one runtime name at a time.
- docs/PRD_TODO_AGENT_WORKBOARD.md describes a broader todo/workboard/evidence system that needs runtime-neutral sessions.
- Project Workspaces already needs runtime/client counts, so runtime identity must become explicit before more UI surfaces depend on it.
- #34 covered a broader rename plus Codex detection direction, but is closed and does not match the current compatibility-preserving runtime adapter approach.
- #38 only covers missing Codex sessions directory no-op and is too narrow for the shared architecture.
- #41/#43 cover project identity and project APIs, not the runtime adapter or work item evidence contract.

The implementation should use Adapter/Strategy plus a registry. Runtime-specific parsers own source quirks; application services consume normalized AgentSession and RuntimeSession records.

## Domain Contracts

Add or formalize these concepts:

~~~ts
type WorkItemStatus = 'inbox' | 'planned' | 'active' | 'blocked' | 'done' | 'archived'
type WorkItemKind = 'todo' | 'idea' | 'note' | 'project_task'

interface WorkItem {
  id: string
  kind: WorkItemKind
  status: WorkItemStatus
  title: string
  body?: string
  projectRoot?: string
  area?: string
  createdAt: Date
  updatedAt: Date
  completedAt?: Date
  statusSource: 'user' | 'accepted_agent_suggestion'
}

interface AgentSession {
  id: string
  runtimeId: RuntimeId
  runtimeSessionId: string
  projectRoot?: string
  cwd: string
  status: SessionStatus | 'unknown'
  title: string
  lastActiveAt: Date
  evidenceSummary?: string
}

interface WorkItemSessionLink {
  workItemId: string
  agentSessionId: string
  linkSource: 'user' | 'accepted_agent_suggestion' | 'heuristic_suggestion'
  acceptedAt?: Date
}

interface ProgressEvidenceBase {
  id: string
  runtimeId?: RuntimeId
  kind: 'message' | 'tool_call' | 'file_change' | 'plan_event' | 'test_result'
  summary: string
  sourcePath?: string
  occurredAt: Date
  confidence: 'explicit' | 'inferred'
}

type ProgressEvidence =
  ProgressEvidenceBase &
    (
      | { workItemId: string; agentSessionId?: string }
      | { workItemId?: string; agentSessionId: string }
    )
~~~

Rules:

- AgentSession.id is the global stable session ID used by persistence, links, and APIs. It must be derived from runtimeId plus runtimeSessionId and must satisfy the shared session ID validator/storage constraints. Use a validator-compatible encoded form such as `${runtimeId}_${runtimeSessionId}` after normalizing both parts to the allowed `[A-Za-z0-9_-]` alphabet; do not introduce an unaccepted delimiter such as `:` without updating the validator and all storage/API call sites together. runtimeSessionId remains the raw runtime-local identifier.
- ProgressEvidence must be attachable. Each record requires at least one stable attachment anchor: workItemId or agentSessionId.
- User-visible task status changes require statusSource user or accepted_agent_suggestion.
- Inferred evidence can suggest progress but cannot mark planned/done by itself.
- No evidence means blank progress, not fake completion.

## Runtime Contracts

Add a runtime domain module, for example src/domain/runtime/.

~~~ts
type RuntimeId = 'claude-code' | 'codex' | 'cursor' | (string & {})
type RuntimeKind = 'cli' | 'ide' | 'cloud' | 'unknown'

type RuntimeCapability =
  | 'session-history'
  | 'process-scan'
  | 'resume'
  | 'quota'
  | 'plans'
  | 'hooks'

interface RuntimeDescriptor {
  id: RuntimeId
  displayName: string
  kind: RuntimeKind
  executableNames: string[]
  sessionPathHints: string[]
  capabilities: RuntimeCapability[]
}

interface RuntimeSession {
  runtimeId: RuntimeId
  sessionId: string
  sourcePath?: string
  cwd: string
  status: SessionStatus | 'unknown'
  title: string
  initialPrompt?: string
  lastMessage?: string
  lastTool?: string
  lastToolInput?: Record<string, unknown>
  currentFile?: string
  filesTouched: string[]
  toolCount: number
  messageCount: number
  startedAt?: Date
  lastActiveAt: Date
  completedAt?: Date
  usageStats?: RuntimeUsageStats
  runtimeMetadata?: Record<string, string | number | boolean | null>
}

interface RuntimeUsageStats {
  inputTokens?: number
  outputTokens?: number
  cacheReadTokens?: number
  cacheWriteTokens?: number
  totalTokens?: number
  totalCostUsd?: number
  model?: string
}

interface RuntimeCommand {
  executable: string
  args: string[]
  cwd?: string
}

interface RuntimeScanOptions {
  maxAgeDays?: number
  mode?: 'basic' | 'full'
  projectRoot?: string
  includeArchived?: boolean
}

interface RuntimeScanError {
  runtimeId: RuntimeId
  code: 'missing-root' | 'read-failed' | 'parse-failed' | 'unsupported-schema' | 'unknown'
  message: string
  sourcePath?: string
  recoverable: boolean
}

interface RuntimeScanResult {
  runtime: RuntimeDescriptor
  sessions: RuntimeSession[]
  errors: RuntimeScanError[]
}

interface AgentRuntimeAdapter {
  descriptor: RuntimeDescriptor
  scanSessions(options?: RuntimeScanOptions): Promise<RuntimeScanResult>
  buildResumeCommand?: (session: RuntimeSession) => RuntimeCommand | undefined
}
~~~

Do not return shell command strings from buildResumeCommand().

## Runtime Adapters

### Claude Code

Use the existing Claude Code scanner/parser as a wrapped source:

- Source hints: ~/.claude/projects/<project>/<session>.jsonl
- Capabilities: session-history, process-scan, resume, quota, plans, hooks
- Runtime ID: claude-code

The adapter maps existing parsed sessions to RuntimeSession. The old parser does not need to be rewritten in the first slice.

### Codex

Use rollout JSONL files:

- Source hints: ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl
- Runtime ID: codex
- Capabilities: session-history, process-scan, resume
- Expected records: session_meta, event_msg, response_item
- Required fields: session_meta.payload.id, session_meta.payload.cwd
- Resume command: buildResumeCommand(session) returns `{ executable: 'codex', args: ['resume', session.sessionId], cwd: session.cwd }`. It must use the raw RuntimeSession.sessionId, not the encoded AgentSession.id.

Parser behavior:

1. Parse JSONL line by line.
2. Use session_meta.payload.id as sessionId.
3. Use session_meta.payload.cwd as cwd.
4. Treat both response_item.payload.type === "message" with role === "user" and event_msg.payload.type === "user_message" as user message sources. Use the first user message from either source as initialPrompt and derived title.
5. Track assistant/user message counts and function call counts.
6. Track file hints from known tool input keys such as path, file_path, filePath, notebook_path.
7. Store runtime-specific metadata such as originator, cli_version, source, thread_source, model_provider.
8. Throw structured parse errors for bad files; adapter catches them into RuntimeScanResult.errors.

Missing ~/.codex/sessions should be no-op. Bad files inside an existing root should not hide good sessions.

## Runtime Registry

~~~ts
class RuntimeRegistry {
  register(adapter: AgentRuntimeAdapter): void
  get(runtimeId: RuntimeId): AgentRuntimeAdapter | undefined
  list(): AgentRuntimeAdapter[]
  scanAll(options?: RuntimeScanOptions): Promise<RuntimeScanResult[]>
}
~~~

scanAll() must isolate adapter failures. A failed adapter must return a RuntimeScanResult for that runtime with an empty sessions array and structured RuntimeScanError entries; it must not reject the whole scan or hide healthy sessions from other runtimes. Implementations should use Promise.allSettled or equivalent per-adapter error boundaries.

Default registry:

~~~text
ClaudeCodeRuntimeAdapter
CodexRuntimeAdapter
~~~

Future registry:

~~~text
CursorRuntimeAdapter
Other CLI/IDE/cloud adapters
~~~

## Preferred File Layout

~~~text
src/domain/runtime/
  index.ts
  types.ts

src/adapters/runtimes/
  claude-code.ts
  codex.ts
  registry.ts
  index.ts
~~~

If local guardrails block adding multiple adapter files during a first pass, keep the implementation localized and record the split as a follow-up. Do not let the temporary layout become the architecture.

## Integration Slices

### Slice 1: SpecRail Artifacts

- Create issue #44.
- Add specs/GH44/product.md.
- Add specs/GH44/tech.md.
- Validate formatting and link paths.

### Slice 2: Work Item Foundation

- Add work_items and areas persistence if the app does not already have durable equivalents.
- Add CRUD API for Inbox/Todo/Idea capture.
- Add basic Overview/Todo/Inbox UI surfaces.
- Enforce manual status transitions and accepted agent suggestions.

### Slice 3: Runtime Domain and Adapter Foundation

- Add RuntimeDescriptor, RuntimeSession, RuntimeCommand, RuntimeScanResult, AgentRuntimeAdapter.
- Wrap existing Claude Code scanner as ClaudeCodeRuntimeAdapter.
- Add RuntimeRegistry and default registry.
- Add tests for registry IDs and structured resume command.

### Slice 4: Codex Runtime Adapter

- Add CodexRuntimeAdapter for rollout JSONL.
- Add missing-directory no-op behavior.
- Add bad-file isolation.
- Add runtime badge/filter support for codex sessions.

### Slice 5: AgentSession and Evidence Linking

- Map RuntimeSession to AgentSession.
- Add work_item_sessions links.
- Add progress_evidence from messages, tool calls, files touched, plan events, and test results.
- Ensure inferred evidence cannot silently change WorkItem status.

### Slice 6: Workboard Projection

- Build Now, Waiting, Stale, Done projections from WorkItem + AgentSession + ProgressEvidence.
- No data should render blank progress, not guessed completion.
- Suggestions must be visibly marked as suggestions until accepted.

### Slice 7: Project Identity and APIs

- Reuse Project Workspaces identity rules: git root, cwd fallback, Unknown project.
- Add or reuse /api/projects for project summaries.
- Add /api/agent-sessions or extend /api/sessions with runtimeId and exact projectRoot filter.
- Prove same-basename projects do not merge.

### Slice 8: Cursor Follow-Up

- Write a separate Cursor discovery spec before implementation.
- Do not guess Cursor storage/API fields in this spec.
- Reuse AgentRuntimeAdapter once source truth is known.

## Testing and Validation

Required implementation commands:

~~~sh
bun run typecheck
bun test
bun run build
cd src/web/client && bun run typecheck && bun run build
git diff --check
~~~

Expected targeted tests:

- Work item can be captured without a project.
- Agent suggestion does not automatically turn an item into planned/done.
- No evidence renders blank progress.
- Claude Code adapter preserves current scanner behavior.
- Codex missing sessions root returns empty sessions and no warning-level user-facing failure.
- Codex rollout JSONL parses into RuntimeSession.
- Codex adapter reports broken rollout files without hiding good sessions.
- Default registry contains claude-code and codex.
- Structured RuntimeCommand is returned for Claude Code resume.
- Structured RuntimeCommand is returned for Codex resume and uses the raw runtime session ID.
- Project filter composes with search, status, runtime, and pagination.
- Same-basename projects do not merge.

SpecRail check:

~~~sh
python3 checks/check_workflow.py --repo . --spec-dir specs/GH44
~~~

If checks/check_workflow.py is absent in the repository, record that explicitly and use git diff --check plus path/link review as the local substitute.

## PR Plan

1. Spec PR: adds only specs/GH44/product.md and specs/GH44/tech.md.
2. Work item foundation PR: durable WorkItem model, CRUD API, Inbox/Todo UI.
3. Runtime foundation PR: runtime domain, adapters, registry, targeted tests.
4. Codex adapter PR: Codex rollout parsing, missing root no-op, parser error visibility.
5. Evidence PR: AgentSession mapping, work_item_sessions, progress_evidence.
6. Workboard PR: Now/Waiting/Stale projection and suggestion gates.
7. Project/API PR: exact projectRoot and runtime filters.
8. Follow-up issue: Cursor adapter discovery spec.

Each PR should reference #44 and avoid mixing unrelated auth, terminal, or project-workspaces changes.

## Risks

- Existing worktrees may carry older brand assumptions. Mitigation: keep compatibility names separate from product boundary language.
- Runtime scan cost can grow with recursive JSONL walking. Mitigation: maxAgeDays, cache by mtime, and basic/full scan modes.
- Bad runtime data can make the dashboard look blank. Mitigation: per-runtime errors plus no silent degradation.
- Codex rollout schema may drift. Mitigation: structural parsing with explicit errors and fixture tests.
- Multiple runtime process matchers may steal sessions from the same cwd. Mitigation: process matching must include runtime identity before live process attribution is shared.
- Workboard inference can overstate progress. Mitigation: explicit confidence and user acceptance gates.

## Open Decisions

- Final package/bin name and migration plan.
- Whether work_items should be added before or after runtime foundation.
- Whether runtime scan errors are returned from /api/sessions, a diagnostics endpoint, or both.
- Whether RuntimeSession should be persisted directly or mapped into the legacy Session entity first.
- Whether project runtime counts should come from session rows or a separate runtime scan summary.
