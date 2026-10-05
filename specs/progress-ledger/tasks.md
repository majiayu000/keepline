# Progress Ledger Task Plan

Issue: https://github.com/majiayu000/keepline/issues/138

Product spec: `specs/progress-ledger/product.md`
Tech spec: `specs/progress-ledger/tech.md`

Tasks are ordered so each one ships something usable. T1 to T3 fix existing
gaps and are valuable even without the ledger.

## Tasks

### SPPL-T1: Transcript facts with tool results and exit codes

Done when Claude and Codex parsers emit `TranscriptFact` streams that pair tool
calls with results, carry exit codes, mark non-mutating calls, and expose Codex
turn lifecycle events, verified against fixtures.

Verify:

```sh
bun test src/__tests__/ledger/facts.test.ts
bun run typecheck
```

### SPPL-T2: Codex desktop liveness and turn-based status

Done when a rollout held open by a Codex process is live, Codex status derives
from the last turn fact (`running`, `stalled`, `idle`, `interrupted`), and
optional read-only enrichment from `state_5.sqlite` and `goals_1.sqlite`
degrades silently when absent.

Verify:

```sh
bun test src/__tests__/ledger/codex-liveness.test.ts
```

### SPPL-T3: Needs-input status from hooks

Done when Claude `PermissionRequest` / `Notification(permission_prompt)` and
Codex `PermissionRequest` set `needs_input` with a reason, the next tool or
prompt event clears it, and install/uninstall round-trips cleanly.

Verify:

```sh
bun test src/__tests__/ledger/needs-input.test.ts src/__tests__/hook.installer.test.ts
```

### SPPL-T4: Ledger storage, config, and retention

Done when migration 015 creates the ledger tables, `ledger.*` config validates
with defaults from the tech spec, and retention deletes rows older than
`retentionDays`.

Verify:

```sh
bun test src/__tests__/ledger/storage.test.ts src/__tests__/config.test.ts
```

### SPPL-T5: Asks and decomposition without a model

Done when asks are extracted verbatim, scope additions are distinguished from
approvals, linked work-item acceptance criteria become items, and the fallback
item is created otherwise.

Verify:

```sh
bun test src/__tests__/ledger/decompose.test.ts
```

### SPPL-T6: Rule matcher, constraints, deviation, progress

Done when steps map to items by anchors, constraint violations are flagged,
deviation thresholds match `off` / `conservative` / `sensitive`, user
corrections win, and progress equals done-with-evidence over total.

Verify:

```sh
bun test src/__tests__/ledger/matcher.test.ts
```

### SPPL-T7: Alerts and native notifications

Done when the alert router raises the four alert kinds with per-kind toggles
and coalescing, broadcasts `ledger:alert`, and sends macOS notifications with
argv-safe arguments.

Verify:

```sh
bun test src/__tests__/ledger/alerts.test.ts
```

### SPPL-T8: Ledger API and web pages

Done when the overview and task comparison pages render live ledgers from the
API, item editing and step correction persist across restart, and settings
toggles are editable in the UI.

Verify:

```sh
bun test src/__tests__/ledger/api.test.ts
bun run typecheck
bun run build
```

### SPPL-T10: Goals and todos inside Keepline

Done when work items support goal/todo hierarchy, outcome, and acceptance
checklists through the existing create and update API with validation; the web
UI can create and edit goals, todos, and checklists; dispatched or
accepted-link sessions get `work_item` requirement items from the checklist;
roll-up computes todo readiness and goal k/n; stale goals are flagged. A test
runs the whole flow with no external upserts.

Verify:

```sh
bun test src/__tests__/ledger/goals.test.ts src/__tests__/work-items.route.test.ts
bun run build
```

### SPPL-T11: Acceptance, follow-ups, and correction rules

Done when finished turns enter `review`; accept, accept-with-gaps, and
follow-up persist; follow-up and correction prompts are generated without a
model call; step corrections can become session rules that the matcher uses.

Verify:

```sh
bun test src/__tests__/ledger/acceptance.test.ts src/__tests__/ledger/rules.test.ts
```

### SPPL-T12: Attribution suggestions and goals view

Done when unlinked sessions get scored suggestions with reasons, confirmation
survives resume and restart, low-confidence sessions stay unattributed, and the
goals page renders goals, todos, live sessions, and stale flags.

Verify:

```sh
bun test src/__tests__/ledger/attribution.test.ts src/__tests__/ledger/api.test.ts
bun run build
```

### SPPL-T13: Review pages, focus mode, alert withdrawal

Done when daily and weekly review endpoints and pages work, the weekly view
reports unattributed runtime share, focus mode queues non-urgent alerts and
summarizes them, cleared alerts are withdrawn, and alerts for the session being
viewed are not notified.

Verify:

```sh
bun test src/__tests__/ledger/review.test.ts src/__tests__/ledger/alerts.test.ts
```

### SPPL-T9: Optional model judge

Done when the judge runs only when enabled, respects interval and backlog
limits, supports `cli-claude`, `cli-codex`, `local`, and `sdk` backends, and
downgrades any `done` without valid evidence to `unverified`. A test proves no
model call happens when disabled.

Verify:

```sh
bun test src/__tests__/ledger/judge.test.ts src/__tests__/ledger/no-network.test.ts
```
