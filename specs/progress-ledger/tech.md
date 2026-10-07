# Progress Ledger Tech Spec

Product spec: `specs/progress-ledger/product.md`

Issue: https://github.com/majiayu000/keepline/issues/138

## Current State (as of e6860ba, 2026-09-17)

Facts this design builds on, with the gaps it must close:

- Runtimes: only `claude-code` and `codex` adapters
  (`src/domain/runtime/types.ts`). Codex reads `~/.codex/sessions/**/rollout-*.jsonl`
  only (`src/adapters/codex/scanner.ts`); it parses messages, function calls,
  and usage, but not tool outputs or turn lifecycle events.
- The process scanner ignores `Codex.app` and `app-server` processes
  (`src/adapters/process/scanner.ts`), so Codex desktop sessions have no process
  and are classified `lost`.
- Status is a time heuristic (`src/adapters/process/detector.ts`): no process is
  `lost`; CPU above threshold or activity under 5 s is `running`; under 30 s is
  `waiting`; otherwise `idle`. Hook-sourced status overrides scans while the
  process is alive (`src/services/session.aggregator.ts`).
- Permission waits are not detected. Claude `Notification` hooks are logged
  only; `Stop` maps to `waiting` (`src/adapters/hook/server.ts`,
  `src/adapters/hook/completion-receiver.ts`).
- Tool-call extraction keeps `{name, input, timestamp}` only. Tool results, exit
  codes, commits, and PRs are not extracted.
- Evidence storage exists: `progress_evidence` (migration 008) with
  `kind IN (message, tool_call, file_change, plan_event, test_result)`,
  `outcome`, `confidence IN (explicit, inferred)`, and `metadata`.
  `work_items`, `agent_sessions`, `work_item_session_links`, `task_dispatches`,
  and `completion_reviews` already link stash work to sessions.
- LLM access exists in `transcript.compressor.ts` (Agent SDK / Anthropic SDK)
  and a loopback-only OpenAI-compatible summarizer, disabled by default
  (`src/lib/config.ts`).
- Notifications are browser-only (`src/web/client/src/hooks/useNotifications.ts`).
- Config is `~/.keepline/config.json`, deep-merged over typed defaults with
  range validation (`src/lib/config.ts`).
- `work_items` (migration 007) has `kind IN (todo, idea, note, project_task)`
  and, since migration 010, `external_source` / `external_id` populated by
  `PUT /api/work-items/external/:source/:externalId`. It has no parent link, no
  outcome, and no checklist. stash's own `WorkItem` has `parentId`, `kind`
  including `epic`, `outcome`, and `checklist`
  (`stash/shared/src/work-item.ts`).
- `work_item_session_links.link_source` already supports
  `heuristic_suggestion` with `acceptance_status = pending`.

Codex transcripts contain what we need: `event_msg` records of type
`task_started`, `task_complete` (with `last_agent_message`), and `turn_aborted`
(with `reason`); tool outputs carry `exit_code` values. On the reference
machine, `lsof` shows the Codex desktop process holding each live rollout file
open, which identifies live desktop sessions without the process-to-cwd match.

## Implementation Scope

- `src/adapters/codex/parser.ts`, `src/adapters/codex/scanner.ts`
- `src/adapters/claude/jsonl.ts`
- `src/adapters/process/scanner.ts`, `src/adapters/process/detector.ts`
- `src/adapters/hook/installer.ts`, `src/adapters/hook/completion-receiver.ts`,
  `src/adapters/hook/server.ts`
- `src/domain/ledger/` (new): types, evidence extraction, matcher, progress
- `src/services/ledger/` (new): decomposer, judge, alert router, retention
- `src/infrastructure/database/migrations/015_progress_ledger.ts` (new)
- `src/infrastructure/notify/macos.ts` (new, CLI-mode fallback)
- `src/adapters/hook/installer.ts`, `src/adapters/hook/spool.ts` (new)
- `menubar-tauri/` (sidecar, autostart, notifications, popover and main
  windows)
- `src/web/api/routes/ledger.ts` (new), `src/web/api/routes/goals.ts` (new),
  `src/web/api/routes/work-items.ts` (hierarchy and checklist fields),
  `src/web/api/server.ts`
- `src/domain/work-item/` (parent, level, outcome, acceptance)
- `src/web/client/src/components/WorkItemsPanel/` (goal, todo, checklist
  editing)
- `src/web/client/src/pages/ledger/` (new)
- `src/lib/config.ts`
- `src/__tests__/ledger/` (new) with transcript fixtures

## Design

### 1. Transcript facts (prerequisite)

Extend both parsers to emit a normalized `TranscriptFact` stream alongside the
existing `RuntimeSession` fields:

```ts
type TranscriptFact =
  | { kind: 'user_message'; text: string; at: string; turnId?: string }
  | { kind: 'agent_message'; text: string; at: string; turnId?: string; final: boolean }
  | { kind: 'turn'; phase: 'started' | 'completed' | 'aborted'; at: string; turnId: string; reason?: string }
  | { kind: 'tool'; callId: string; name: string; input: unknown; at: string;
      mutating: boolean; exitCode?: number; outputHead?: string }
  | { kind: 'limit'; scope: 'usage' | 'budget'; at: string };
```

- Claude: pair `tool_use` with `tool_result` by id; `is_error` and Bash output
  give the exit status.
- Codex: pair `function_call`/`custom_tool_call` with their `*_output` records
  and `item_completed` payloads by call id; read `exit_code`. Map
  `task_started`/`task_complete`/`turn_aborted` to `turn` facts.
- `mutating` is false for a fixed allowlist: sleep/wait, file reads, directory
  listings, search, `git status|diff|log`, and plain `cat|ls|rg|grep|sed -n`.
- Facts are derived on read from the transcript tail and cached in memory by
  `(path, mtime, size)`. They are not persisted.

### 2. Codex desktop and turn status

- Add an open-file liveness probe: one batched `lsof -Fn -c codex` per scan;
  a rollout path held open by any Codex process marks that session live.
- Session status for Codex comes from the last `turn` fact: `started` with a
  live file is `running`; `started` with no live holder and no writes for
  `stalledAfterSeconds` (default 900) is `stalled`; `completed` is `idle`
  (turn done, awaiting next prompt); `aborted` is `interrupted`.
- Optional enrichment: when `~/.codex/state_5.sqlite` exists, read it with
  `mode=ro` for thread titles and `thread_spawn_edges`, and
  `~/.codex/goals_1.sqlite` for goal status (`usage_limited`,
  `budget_limited`, `blocked`). A missing file or unknown schema version is
  ignored and logged once.

### 3. Needs-input status

- Add `needs_input` to `SessionStatus` with a `statusReason` string.
- Installer: add Claude `PermissionRequest`; keep `Notification` and map
  `notification_type` in (`permission_prompt`, `idle_prompt`,
  `agent_needs_input`) to `needs_input`. Add Codex `PermissionRequest`.
- The next `PreToolUse`, `PostToolUse`, or `UserPromptSubmit` for the same
  session clears `needs_input`.
- Without hooks installed, `needs_input` is never inferred from timing.
- Unanswered `AskUserQuestion` and `request_user_input` tool calls are explicit
  input requests in transcript facts; their paired results clear the request.
  `PreToolUse` for these tools also sets `needs_input` instead of `running`.
- Migration 016 adds `last_viewed_turn_id` to `ledger_views`. Viewing records
  the completed turn, independently of notification suppression expiry and
  acceptance. Ordinary unread responses enter `review` without contributing
  progress; read responses with no confirmed criteria move to `ended`.

### 4. Storage (migration 015)

```sql
CREATE TABLE ledger_asks (
  id TEXT PRIMARY KEY, agent_session_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL, text TEXT NOT NULL, authored_text TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('initial', 'addition', 'question')),
  occurred_at TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE requirement_items (
  id TEXT PRIMARY KEY, agent_session_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL, title TEXT NOT NULL,
  anchors TEXT NOT NULL DEFAULT '{}',      -- {paths:[], commands:[], keywords:[]}
  constraints TEXT NOT NULL DEFAULT '[]',  -- [{kind, value}]
  source TEXT NOT NULL CHECK (source IN ('work_item', 'model', 'fallback', 'user')),
  status TEXT NOT NULL CHECK (status IN ('todo', 'doing', 'done', 'unverified')),
  status_source TEXT NOT NULL CHECK (status_source IN ('rule', 'model', 'user')),
  evidence_ids TEXT NOT NULL DEFAULT '[]',
  deleted_by_user INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE ledger_corrections (
  id TEXT PRIMARY KEY, agent_session_id TEXT NOT NULL,
  step_call_id TEXT NOT NULL,
  requirement_item_id TEXT,                -- NULL with accept_off_plan = 1
  accept_off_plan INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);
CREATE TABLE ledger_alerts (
  id TEXT PRIMARY KEY, agent_session_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('needs_input', 'off_plan', 'claimed_unverified', 'stalled', 'limited')),
  detail TEXT NOT NULL, raised_at TEXT NOT NULL, cleared_at TEXT,
  notified INTEGER NOT NULL DEFAULT 0
);
```

Evidence reuses `progress_evidence` with `confidence = 'explicit'` and
`metadata` holding `{callId, command, exitCode, testSummary, commitSha, prUrl}`.
Existing `kind` values suffice: `tool_call` for commands, commits, and PRs;
`test_result` for parsed test summaries; `file_change` for edits. Evidence rows
are written only for steps that matter to the ledger, not for every tool call.

Retention: the existing daemon retention job deletes ledger rows and
ledger-sourced evidence whose session `last_active_at` is older than
`ledger.retentionDays` (default 30).

### 5. Decomposition

- Trigger: a new candidate `user_message`. Keep uncertain intent by default;
  exclude only explicit injections, quoted/pasted reports, pure commands/code,
  short status follow-ups and bare approvals. Strip consecutive leading host
  tag blocks (including nested and self-closing blocks), preserving trailing
  authored text rather than relying on a known-tag list. Preserve questions,
  including Chinese questions ending in “呢”, verbatim with
  `kind=question`, excluding them from progress. `authored_text` holds the input
  outside quoted/code blocks; `text` remains the original displayed message.
- Source a: if the session is linked to a work item (accepted link or dispatch),
  split the work item body's acceptance-criteria list into items.
- Source b: if model recognition is enabled, recognize/decompose the latest
  candidate message once into JSON `{items:[{title, anchors}]}`. Earlier
  processed messages reuse their saved items; older unprocessed candidates
  remain fallback items rather than causing historical model calls. Reject
  anchors not present in the authored text; derive constraints from user words.
- Source c: one item titled with the first 120 characters of the ask.
- Re-decomposition replaces items with `source != 'user'` and keeps user items
  and corrections.

### 6. Rule matcher (always on, zero tokens)

- For each mutating step, score items by anchor hits: path prefix or glob match
  on edited files, command regex, keyword overlap with the step summary. The
  best-scoring item above a threshold wins; ties go to the most recent item.
- Constraint checks run per step: `no_public_api_change` flags diffs that add,
  remove, or modify `pub` items (Rust) or `export` declarations (TS/JS);
  `path_forbidden` flags edits under listed globs.
- Completion is independent of attribution: each command anchor is a literal
  execution criterion, not a regex proof. Every command in an item must have
  its own latest successful result from a directly recorded Bash/exec_command
  call. Both the tool exit code and any observed test summaries must succeed.
  Text printed by echo, source code containing an unexecuted nested call, and
  shell control-flow wrappers do not prove the anchored command ran.
- A newer failed or pending attempt replaces the older success for the same
  command, even when a correction assigns that attempt to another item.
  An observed file edit or a later mutating execution wrapper without a
  direct command receipt invalidates prior automatic checks for the session.
  Without a complete dependency graph or nested execution receipts, this
  conservatively requires rechecking; wrapper text never grants success.
  Historical evidence stays in the trail, but only current evidence counts.
  Existing explicit user status decisions remain distinct from rule status.
- An item becomes `done` only when all its command criteria have current
  evidence; matched work without successful checks is `doing`. A check
  invalidated by an observed edit or uncertain execution is `unverified`
  until it runs again.
- Existing cached facts and computed ledgers use new fingerprint versions so
  a deployment recomputes old completion results even without a file append.
  Computed ledger fingerprints and startup cleanup share one version from
  the infrastructure cache, preserving current-day scan/detail rows across
  processes while discarding rows from older versions or days.
  Scanner fallback imports are lazy; the resident service's static dependency
  graph must continue to exclude app-only usage/pricing code.
- Deviation: `conservative` raises off-plan after at least 8 consecutive
  unmatched mutating steps spanning at least 10 minutes; `sensitive` after 3
  steps. Constraint violations always raise when constraint checks are on.
  `ledger_corrections` with `accept_off_plan` suppress that run.

### 7. Requirement recognition (optional)

- Backends: `cli-claude` (`claude -p --output-format json`), `cli-codex`
  (`codex exec --json`), `local` (existing loopback OpenAI-compatible
  summarizer), `sdk` (existing Agent SDK path). Default: disabled.
- Trigger: latest new candidate user message only, at most one in flight per
  session. An atomic claim in `ledger_judgments`, keyed by `(agent_session_id,
  ask_id)`, deduplicates scanner/HTTP processes and restarts before invoking a
  provider. Store `judged_at` and recognized `items` JSON there; a null result
  retains the fallback and does not retry automatically.
- Input: the current authored user message, plus bounded previous user context.
  No tool output, execution evidence, agent report or completion claim is sent.
- Output JSON: `{items:[{title,anchors:{paths,commands,keywords}}]}`; an empty
  items array rejects a non-requirement candidate. Model status/evidence fields
  are ignored. Actual evidence matching runs after decomposition.
- User edits/removals override derived items across rescans and AI toggles.
  Explicit re-decomposition may retry; ordinary tools and reports never do.

### 8. Alerts and notifications

- `AlertRouter` evaluates after each sync and hook event, writes
  `ledger_alerts`, and coalesces per `(session, kind)` within
  `alertCoalesceSeconds` (default 600).
- Channels: existing WebSocket broadcast (`ledger:alert`), consumed by the web
  UI and by the app, which turns it into a native notification (section 14a).
- Each kind has its own toggle. `claimed_unverified` fires only on turn
  completion.

### 9. Config

```jsonc
"ledger": {
  "enabled": true,
  "retentionDays": 30,
  "judge": { "enabled": false, "backend": "cli-claude", "model": null },
  "deviation": "conservative",          // off | conservative | sensitive
  "constraints": true,
  "alerts": { "needs_input": true, "off_plan": true,
              "claimed_unverified": true, "stalled": true, "limited": true },
  "nativeNotifications": true,
  "focus": { "minutes": 30, "until": null },
  "staleGoalDays": 7,
  "exclude": { "projects": [], "runtimes": [] }
}
```

Validated by the existing `ConfigManager`; exposed through
`GET/PUT /api/settings/ledger`.

### 10. API and UI

- `GET /api/ledger?hours=` overview rows.
- `GET /api/ledger/:sessionId` ledger detail: asks, items, trail, claims.
- `PUT /api/ledger/:sessionId/items` replace user-edited items.
- `POST /api/ledger/:sessionId/corrections` step reassignment or off-plan
  acceptance.
- `POST /api/ledger/:sessionId/redecompose`.
- `POST /api/ledger/:sessionId/acceptances` with `decision`, optional
  `droppedItemIds` and `reason`.
- `GET /api/ledger/:sessionId/follow-up` and
  `GET /api/ledger/:sessionId/correction?runId=` return prompt text only.
- `POST /api/ledger/:sessionId/attribution` confirms, replaces, or clears a
  link (`workItemId` or `none`).
- `GET /api/goals?area=` goal rows with child todos and live session state;
  `GET /api/ledger/review?date=` and `?week=` for the review pages;
  `POST /api/goals/todos/:id/complete` and
  `POST /api/ledger/:sessionId/carry-over` (creates or updates a Keepline
  todo).
- UI: Overview, Goals, Task ledger, Review, and Settings pages matching the
  design canvas. Live updates ride the existing `/ws` channel.

### 11. Acceptance, follow-ups, corrections

- `ledger_acceptances` (migration 015):
  `id, agent_session_id, turn_id, decision CHECK (decision IN ('accepted',
  'accepted_with_gaps', 'follow_up')), dropped_item_ids TEXT, reason TEXT,
  created_at`. A session's user-facing state is derived: last turn completed
  and no acceptance for it → `review`; latest decision `accepted` or
  `accepted_with_gaps` → `accepted`; `follow_up` → back to `running` when the
  next turn starts.
- Follow-up and correction prompts are deterministic templates over remaining,
  unverified, and dropped items plus constraint violations. No model call.
  The API returns text; the client copies it. Nothing is sent to an agent.
- `ledger_rules`: `id, agent_session_id, matcher TEXT` (`{paths, commands}`),
  `requirement_item_id`, `created_at`. Created when a step correction is saved
  with "apply to similar steps". The rule matcher consults these before anchors.

### 12. Goals and todos (Keepline-native)

- Keepline is the system of record for goals and todos in this tranche. stash
  is optional: when present, it can feed the same rows through the external
  upsert, but no behavior depends on it.
- Migration 015 adds to `work_items`: `parent_id TEXT`, `level TEXT` (`goal` or
  `task`, validated in code because SQLite cannot alter the existing `kind`
  CHECK), `outcome TEXT`, `acceptance TEXT` (JSON array of
  `{id, text, completed}`).
- Extend `POST /api/work-items` and `PATCH /api/work-items/:id` with
  `parentId`, `level`, `outcome`, and `acceptance`, with validation: a goal has
  no parent; a todo's parent must be a goal; checklist items need non-empty
  text. Deleting a goal requires moving or deleting its todos first.
- Extend the existing `WorkItemsPanel` (or a new Goals page reusing its hooks)
  to create and edit goals, todos, and checklists inline.
- Optional, later: accept the same fields on
  `PUT /api/work-items/external/stash/:externalId` (`parentExternalId`,
  `level` from stash `epic`, `outcome`, `checklist`). Write-back to stash is out
  of scope.
- Requirement items with `source = 'work_item'` are created from `acceptance`
  when a session is linked by dispatch or accepted link, and stay in sync with
  later checklist edits.
- Roll-up service: a todo's checklist item is satisfied when any linked
  session has an item for it with status `done` and an accepted turn. When all
  are satisfied, the todo gets a `ready_to_complete` flag that the UI turns
  into a prompt. Goal progress = child todos with status `done` over all child
  todos; `active` count = todos with a running linked session.
- Stale goal: no child todo status change, no linked session activity, and no
  edit for 7 days (`ledger.staleGoalDays`).

### 13. Attribution

- On first sight of an unlinked session, score open todos by: same project
  root (strong), ask mentions the todo title or an id token like `T1` or
  `cove-183` (strong), keyword overlap with title and checklist (weak), and the
  todo being `active` (weak). Above the threshold, write a
  `work_item_session_links` row with `heuristic_suggestion` / `pending` and the
  reasons in metadata. Below it, the session stays unattributed.
- Confirmation flips the link to `accepted`. Resumed and compacted sessions
  keep the link because it is keyed by runtime session id.
- Unattributed share for the weekly view: sum of turn durations (turn start to
  turn end or last write) for sessions without an accepted link, divided by the
  sum for all sessions in the week.

### 14a. Native notification channel

Native notifications are delivered by the app (section 15). When the service
runs without the app (CLI or daemon mode), it falls back to
`osascript -e 'display notification ...'` with argv-passed arguments; that
fallback cannot open the window on click.

### 14. Alert routing additions

- Focus mode: `ledger.focus.until` timestamp; while set, only `needs_input`
  notifies; others are queued and summarized in one notification at expiry.
- Withdrawal: when an alert's condition clears, `cleared_at` is set and the
  native notification is removed where the OS allows; the WebSocket emits
  `ledger:alert-cleared`.
- Suppression: the client reports the session it is displaying; alerts for
  that session are recorded but not notified.
- `needs_input` inferred without hooks is a display-only `possibly_waiting`
  flag and never routes to notifications.

### 15. App shell (Tauri, extends `menubar-tauri`)

- Base: `menubar-tauri` is Tauri v2 with `tray-icon`, dock visibility, and
  window resize already in place; today it reads quota files only and does not
  talk to the service.
- Embedded service: ship `dist/keepline-service` (`bun run
  build:embedded-service`) as a Tauri sidecar (`bundle.externalBin`). On
  launch, probe the loopback health endpoint; attach to a compatible running
  service, otherwise spawn the sidecar and record that the app owns it. Mirror
  the lifecycle stash Time Ledger already uses for the same binary.
- Auth: the app obtains the local API token from the service it owns or
  attaches to and injects it into its webviews; the user never handles it.
- Windows: a tray popover webview at the client route `/menubar` and a main
  webview for the full UI. Both load the existing React client; no second UI
  codebase. The tray title shows counts pushed over `/ws`.
- Login start: `tauri-plugin-autostart` (LaunchAgent), on by default.
- Notifications: deliver with `tauri-plugin-notification`. Click-to-open on
  macOS desktop must be verified in a spike; if the plugin cannot report the
  click, use a small native bridge (`UNUserNotificationCenter` delegate) behind
  the same interface. The payload carries `{sessionId, anchor}` for deep links.
- First version: local builds signed for the author's machine; no updater, no
  crash reporter.

### 16. Hook spool

- Replace the installed `curl` command with a small `keepline-hook` script
  installed under `~/.keepline/bin/`. Each event (id, runtime, payload, received
  time) is fsynced to a private temporary file, then atomically renamed into
  `~/.keepline/spool/events/<id>.json`. Independent event files require no
  shared lock and cannot drop events on lock contention or abandoned locks.
  HTTP delivery is detached with a 1-second timeout; the hook always exits 0.
- The service ingests the spool at startup and on each scan, deduplicating by
  event id, and removes delivered files. It also drains existing JSONL spool
  records independently. Transient failures remain retryable; malformed or
  permanently rejected records are preserved under `spool/rejected/` and do
  not block later events. Ingested events use the same receivers as live POSTs.
- The installer keeps using `KEEPLINE_HOOK_MARKER`; uninstall removes the
  script entries and leaves the spool for the service to drain.

## Verification

- Fixture transcripts under `src/__tests__/ledger/fixtures/` for Claude CLI,
  Codex CLI, and Codex desktop, with known exit codes, tests, a commit, a PR
  URL, a permission request, an abort, and an off-plan run.
- Unit tests per stage: fact extraction, liveness, needs-input mapping, matcher,
  deviation thresholds, requirement recognition and message deduplication (with a stubbed backend), alert
  coalescing, retention, config validation.
- A no-network test asserts no subprocess or HTTP model call when
  `judge.enabled` is false.
- `bun run typecheck` and the architecture import test stay green.

## Risks And Rollback

- **Transcript format drift.** Parsers degrade per record: an unknown record is
  skipped and counted; a session with too many unknown records shows "ledger
  unavailable" instead of wrong progress.
- **lsof cost.** One batched call per scan; disable with
  `ledger.enabled = false` or by excluding the Codex runtime.
- **Model cost or leakage.** Off by default; one persisted attempt per new
  candidate user message caps calls. Input is authored user text plus bounded
  user context; execution output and agent reports are not sent.
- **Hook churn.** New hook entries use the existing marker and are removed by
  `keepline hooks uninstall`.
- **Rollback.** `ledger.enabled = false` stops all ledger work; migration 015
  only adds tables, so older builds ignore them.
