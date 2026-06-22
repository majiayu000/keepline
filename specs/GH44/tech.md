# Runtime-Neutral Agent Orchestration Tech Spec

Product spec: specs/GH44/product.md

Issue: https://github.com/majiayu000/claude-hub/issues/44

## Context

- AGENTS.md and README.md have historically described the product through one runtime name at a time.
- specs/GH44/product.md defines the Todo + runtime session + evidence Workboard behavior that needs runtime-neutral sessions.
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
  acceptanceStatus: 'accepted' | 'pending' | 'rejected'
  acceptedAt?: Date
}

interface ProgressEvidenceBase {
  id: string
  runtimeId?: RuntimeId
  kind: 'message' | 'tool_call' | 'file_change' | 'plan_event' | 'test_result'
  outcome?: 'progress' | 'blocked' | 'completed' | 'failed'
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

- AgentSession.id is the global stable session ID used by persistence, links, and APIs. It must be derived from raw runtimeId plus raw runtimeSessionId, must satisfy the shared session ID validator/storage constraints, and must be collision-resistant within the current 64-character validator limit. Use a validator-compatible fixed form such as `${safeRuntimeId.slice(0, 24)}_${sha256(runtimeId + "\0" + runtimeSessionId).slice(0, 32)}` where safeRuntimeId contains only `[A-Za-z0-9_-]`. Do not rely on lossy normalization alone. runtimeSessionId remains the raw runtime-local identifier.
- WorkItemSessionLink acceptanceStatus is authoritative. User-created and accepted_agent_suggestion links are accepted and should set acceptedAt; heuristic_suggestion links start pending and cannot affect Workboard buckets until accepted.
- ProgressEvidence must be attachable. Each record requires at least one stable attachment anchor: workItemId or agentSessionId.
- ProgressEvidence.outcome is the typed signal for completion/blocking/failure. Bucket logic must not parse free-text summary to infer done state.
- User-visible task status changes require statusSource user or accepted_agent_suggestion.
- Inferred evidence can suggest progress but cannot mark planned/done by itself.
- No evidence means blank progress, not fake completion.

## Runtime Contracts

Add a runtime domain module, for example src/domain/runtime/.

~~~ts
type RuntimeId = 'claude-code' | 'codex' | 'cursor' | (string & {})
type RuntimeKind = 'cli' | 'ide' | 'cloud' | 'unknown'
type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue }

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
  projectRoot?: string
  agentId?: string
  parentSessionId?: string
  isSubAgent?: boolean
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
  runtimeMetadata?: Record<string, JsonValue>
}

interface RuntimeUsageStats {
  totalInputTokens: number
  totalOutputTokens: number
  totalTokens: number
  totalCost: number
  apiCalls: number
  cacheReadTokens?: number
  cacheWriteTokens?: number
  model?: string
}

interface RuntimeCommand {
  executable: string
  args: string[]
  cwd?: string
}

type RuntimeRecoveryMethod = 'resume' | 'continue' | 'new'

interface RuntimeRecoveryOptions {
  method: RuntimeRecoveryMethod
  initialPrompt?: string
  skipPermissions?: boolean
}

interface RuntimeScanOptions {
  maxAgeDays?: number
  mode?: 'basic' | 'full'
  /** Exact ProjectIdentity.rootPath filter, applied after cwd is resolved to a project root. */
  projectRoot?: string
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
  /** Required when descriptor.capabilities includes resume; absent or undefined otherwise. */
  buildRecoveryCommand?: (session: RuntimeSession, options: RuntimeRecoveryOptions) => RuntimeCommand | undefined
}
~~~

Do not return shell command strings from buildRecoveryCommand().

Runtime rules:

- RuntimeScanOptions.projectRoot is applied after resolving RuntimeSession.cwd to ProjectIdentity.rootPath, not by comparing raw cwd. It is an exact filter over the normalized RuntimeSession.projectRoot, not a display path, basename, or text search. Adapters must resolve projectRoot with the shared Project Workspaces identity rules: git root first; normalized cwd fallback even when the historical directory no longer exists; Unknown only for empty or unnormalizable cwd strings. If an adapter cannot resolve project roots itself, the registry or API layer must apply this filter after normalization rather than dropping nested cwd sessions.
- RuntimeSession.projectRoot is the resolved project root used by project filters and project counts. It may differ from cwd when a session runs inside a nested directory.
- Archived runtime sessions are out of scope for the first runtime-neutral contract because no supported adapter has a documented archived session source root. Do not expose includeArchived or an archived RuntimeSession flag until each participating adapter declares its archived source hints and normalized recovery behavior.
- Any adapter whose descriptor.capabilities includes resume must implement buildRecoveryCommand() and return a RuntimeCommand for RuntimeRecoveryOptions.method === "resume" when a session is resumable. The same builder must preserve existing continue/new recovery modes and skipPermissions flags when that runtime supports them. Adapters without resume capability must not expose a resume action. Registry registration or tests should fail on a resume capability without a command builder.

## Runtime Adapters

### Claude Code

Use the existing Claude Code scanner/parser as a wrapped source:

- Source hints: ~/.claude/projects/<project>/<session>.jsonl
- Capabilities: session-history, process-scan, resume, quota, plans, hooks
- Runtime ID: claude-code
- Recovery commands: buildRecoveryCommand(session, { method: 'resume' }) returns `{ executable: 'claude', args: ['--resume', session.sessionId], cwd: session.cwd }`; `{ method: 'continue' }` returns args `['--continue']`; `{ method: 'new', initialPrompt }` starts a new Claude command with the prompt. skipPermissions appends `--dangerously-skip-permissions` before the method-specific args.

The adapter maps existing parsed sessions to RuntimeSession. The old parser does not need to be rewritten in the first slice, but the wrapper must preserve agentId, parentSessionId, isSubAgent, and usageStats. Per-file read/parse failures from Claude JSONL must be surfaced as RuntimeScanResult.errors instead of only being logged.

### Codex

Use rollout JSONL files:

- Source hints: ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl
- Runtime ID: codex
- Capabilities: session-history, process-scan, resume
- Expected records: session_meta, turn_context, event_msg, response_item
- Required fields: session_meta.payload.id, session_meta.payload.cwd
- Recovery commands: buildRecoveryCommand(session, { method: 'resume' }) returns `{ executable: 'codex', args: ['resume', session.sessionId], cwd: session.cwd }`; `{ method: 'continue' }` returns args `['resume', '--last']`; `{ method: 'new', initialPrompt }` starts a new Codex command with the prompt. skipPermissions appends `--dangerously-bypass-approvals-and-sandbox` before the session id, `--last`, or prompt. Resume must use the raw RuntimeSession.sessionId, not the encoded AgentSession.id.

Parser behavior:

1. Parse JSONL line by line.
2. Use session_meta.payload.id as sessionId.
3. Use session_meta.payload.cwd as cwd.
4. Accept turn_context records as contextual metadata. Do not treat them as unsupported schema; store relevant stable fields such as cwd, approval_policy, sandbox_policy, model, and effort in runtimeMetadata. session_meta.payload.cwd remains the authoritative cwd.
5. Treat both response_item.payload.type === "message" with role === "user" and event_msg.payload.type === "user_message" as user message sources. Use the first user message from either source as initialPrompt and derived title.
6. Track assistant/user message counts and function call counts.
7. Track file hints from known tool input keys such as path, file_path, filePath, notebook_path.
8. Store runtime-specific metadata such as originator, cli_version, source, thread_source, model_provider.
9. Throw structured parse errors for bad files; adapter catches them into RuntimeScanResult.errors.

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

register() must reject duplicate adapter descriptor.id values. RuntimeId is the key for links, filters, errors, and get(runtimeId); replacing or shadowing an existing adapter is a configuration error.

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
- Bucket predicates are projections, not status mutations:
  - Evaluate each work item into at most one bucket using first-match precedence: archived work items are hidden from active buckets by default; then Done; then Waiting; then Stale; then Now; remaining inbox/planned items stay unbucketed until they have status or evidence.
  - Done: WorkItem.status is done. A completed AgentSession can provide evidence but cannot move a work item into Done without user or accepted_agent_suggestion statusSource.
  - Waiting: WorkItem.status is blocked, or the most recent accepted linked AgentSession.status is waiting and there is no newer ProgressEvidence.outcome === "completed" for the same work item/session.
  - Stale: WorkItem.status is planned, active, or blocked and neither accepted linked AgentSession.lastActiveAt nor ProgressEvidence.occurredAt is within the configured stale window. Done and archived work items are never stale.
  - Now: WorkItem.status is active, or planned with an accepted WorkItemSessionLink to a running/waiting AgentSession.
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
- Codex turn_context records are accepted and preserved as metadata without replacing session_meta identity.
- Codex adapter reports broken rollout files without hiding good sessions.
- Default registry contains claude-code and codex.
- Registry rejects duplicate runtime adapter IDs.
- Structured RuntimeCommand is returned for Claude Code resume, continue, and new recovery modes.
- Structured RuntimeCommand is returned for Codex resume, continue, and new recovery modes and resume uses the raw runtime session ID.
- Registry rejects or test-fails an adapter that declares resume capability without buildRecoveryCommand.
- RuntimeScanOptions.projectRoot filters by exact resolved projectRoot, including nested cwd sessions.
- Missing historical cwd strings remain normalized cwd project roots; only empty or unnormalizable cwd maps to Unknown.
- Archived runtime session scanning is not exposed until adapter-specific archived source roots and recovery behavior are specified.
- Workboard buckets follow the Now, Waiting, Stale, Done predicates without mutating WorkItem.status.
- WorkItemSessionLink acceptanceStatus gates Workboard bucket membership.
- ProgressEvidence.outcome drives completed/blocking/failure evidence without parsing summary text.
- RuntimeSession preserves Claude sub-agent fields agentId, parentSessionId, and isSubAgent.
- RuntimeSession.usageStats maps losslessly to existing SessionUsageStats, including apiCalls.
- RuntimeSession.runtimeMetadata accepts structured JSON values.
- Claude adapter wrapper reports per-file read/parse failures in RuntimeScanResult.errors.
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
