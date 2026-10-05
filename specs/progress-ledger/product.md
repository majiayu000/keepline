# Progress Ledger Product Spec

Issue: https://github.com/majiayu000/keepline/issues/138

Design canvas: https://claude.ai/artifact/14tdZ4LQb5WNmoCtQtQytX (private; pages
"体验框架", "核心界面", "流程与状态")

## Summary

A developer runs several local coding agents in parallel (Claude Code, Codex
CLI/desktop, later cursor-agent and opencode) in service of long-term goals that
are broken into short-term todos. Keepline already shows which sessions exist
and whether a process is alive. It does not answer the questions that matter
while delegating:

- What did I originally ask this agent to do, including what I added later?
- What is it actually doing right now, and does that match my ask?
- How far along is it, measured by evidence rather than its own report?
- Which agents need me, and why?
- How far along is each long-term goal, given the work all agents did for it?

The Progress Ledger adds an "ask vs actual" layer on top of the existing
session pipeline and rolls it up through todos to long-term goals. Progress is
computed from **requirement items** backed by **evidence** mechanically
extracted from transcripts and confirmed by the user's **acceptance**. Model
judgment is optional, rate-limited, and never allowed to invent evidence.

## Concepts

1. **Goal**: a long-term outcome (weeks to months), a Keepline work item with
   level `goal` whose `outcome` states the success criteria.
2. **Todo**: a short-term deliverable (hours to days), a Keepline work item
   whose parent is a goal and whose acceptance checklist holds the criteria.
3. **Session**: one agent conversation. A todo may have many sessions across
   agents. A session is linked to at most one todo.
4. **Ask**: the user's own words in a session, verbatim, ordered by time.
5. **Requirement item**: one checkable unit for a session. Items carry anchors
   (paths, commands, keywords) and optional constraints. Users can edit them.
6. **Evidence**: facts read from the transcript without interpretation:
   commands with exit codes, test summaries, file edits, commits, PR output,
   sub-agent verdicts. Agent prose is never evidence.
7. **Step**: one mutating tool call. Non-mutating calls (sleep, wait, reads,
   listings, status queries) are not steps.
8. **Off-plan**: a run of steps that map to no requirement item, or a step that
   violates a declared constraint.
9. **Acceptance**: the user's confirmation that a finished turn did what was
   asked. Only the user moves work to "accepted".

## Product Behavior

### Session ledger

1. Keepline builds a ledger for every Claude Code and Codex session active in
   the retention window, without requiring the user to launch agents through
   Keepline. Codex desktop sessions are included.
2. The ledger shows the ask verbatim. Keepline never rewrites the user's words.
3. Requirement items come from, in priority order:
   a. the acceptance checklist of the linked todo;
   b. model decomposition of the ask, when model judgment is enabled;
   c. a single item equal to the whole ask, as a fallback.
   Each item shows its source (todo, model, added later, edited by you).
4. Each item shows a status: `done`, `doing`, `todo`, or `unverified`, and the
   provenance of that status (rule, model, you).
5. The trail groups steps by turn and tags each step with its requirement item
   or `off-plan`. Read-only and waiting calls are collapsed into a count.
   Selecting an item filters the trail to its steps.
6. A "claims vs evidence" table lists statements from each turn's final agent
   message next to matching evidence or "no evidence found", with an action to
   generate a request for that evidence.
7. Constraints ("do not change public API", "keep old failed tags") are shown
   separately with their rule-check result.

### Session lifecycle and acceptance

8. User-facing task states: `running`, `needs_input`, `review` (turn finished,
   awaiting the user), `accepted`, `stopped` (stalled, interrupted, or
   limited). Off-plan is an overlay on `running`, not a state.
9. In `review`, the user can: accept; accept with gaps (remaining items marked
   "dropped by you" with a reason); or generate a follow-up.
10. A follow-up is text built from remaining and unverified items, shown in an
    editable dialog, and copied to the clipboard ("copy" or "copy and jump to
    agent"). Keepline never sends it to the agent.
11. Off-plan runs collapse into one group with two actions: "reasonable, stop
    alerting" or "drifted, generate a correction prompt" (same copy-only flow).
12. Corrections persist and win over computed values: edit items, reassign a
    step to an item, accept an off-plan run. A step reassignment can optionally
    become a rule for the session ("later edits to `Cargo.*` belong to item 6").

### Goals, todos, attribution

13. Goals and todos live in Keepline, extending its existing work items. The
    user can create and edit goals, todos, and acceptance checklists in
    Keepline. Every behavior in this spec works with no stash installed; stash
    can later feed the same records through the existing external upsert.
14. A new session without a link gets an attribution suggestion (goal › todo)
    with the reasons shown (same repo, ask mentions the todo, matching terms).
    The user confirms, picks another, or marks "no goal". Low-confidence
    sessions stay "unattributed" instead of guessing.
15. Starting an agent from a todo ("hand to Codex / Claude Code") shows the
    prompt first; the todo's checklist becomes the session's requirement items.
    This is the only place Keepline launches an agent, and only on user click.
16. Roll-up: a todo's checklist items are satisfied by evidence from any linked
    session; when all are satisfied after an accepted turn, Keepline asks the
    user to mark the todo done. Goal progress is
    accepted todos over all todos, with in-progress count shown separately. No
    percentage averaging.
17. When an agent's final message proposes follow-up work, Keepline suggests
    adding it as a todo under the same goal; nothing is added without the user.
18. A goals view lists goals with segmented progress (one segment per todo),
    weekly movement, and active agents, and flags goals with no progress for 7
    days. Expanding a goal shows its todos with live session status.
19. The overview can group by urgency (default), goal, or project; each row
    shows its goal › todo path or "unattributed".

### Overview, menubar, review

20. The overview groups rows by urgency: needs you (needs input, off-plan,
    review with gaps), running, accepted today. Reordering pauses while the
    pointer is inside the list. Keyboard: J/K, Enter, O (jump to agent),
    A (accept), C (copy follow-up), / (search).
21. The menubar shows "needs you" and "running" counts and lists those items
    with their primary action. A 30-minute focus mode lets only `needs_input`
    alerts through and delivers the rest as one summary when it ends.
22. A daily review lists still-open work with what remains, accepted work with
    key evidence, and the day's off-plan runs and corrections. "Carry to
    tomorrow" creates or updates a Keepline todo with remaining items as its
    checklist. A weekly view adds per-goal movement and the share of agent
    runtime spent on unattributed sessions.

### Alerts and settings

23. Alerts are raised only for: needs input; off-plan; claimed done with
    unverified items; stopped. Only needs-input plays a sound. Same-kind alerts
    per session coalesce within 10 minutes; simultaneous needs-input alerts
    merge into one. An alert is withdrawn when the condition clears. No alert
    fires for a task the user is currently viewing.
24. Without hooks, "needs input" is shown only as an inferred "possibly waiting"
    state with a dashed outline, and never triggers a notification.
25. Every behavior with cost or noise is a toggle: model judgment (and backend),
    deviation detection (`off` / `conservative` / `sensitive`), constraint
    checks, each alert kind, native notifications, focus duration, per-project
    or per-runtime exclusion.
26. Keepline stores only derived ledger data (requirement items, corrections,
    acceptances, alert state). Transcripts stay where agents keep them. Ledger
    data older than 30 days is deleted automatically (configurable).

### First run and edge states

27. First run shows discovered agents and session counts immediately, then two
    optional steps (enhanced detection via hooks; model judgment), both off by
    default and skippable.
28. Edge states say what is known and offer a next step: no sessions (list the
    paths scanned); unrecognized transcript format (ledger unavailable, status
    only); vague ask (offer to import the ask from the previous session or
    linked todo); process gone without a turn end (no invented cause).

## Non-Goals

- Do not send prompts, approvals, or follow-ups to agents. Launching from a todo
  on explicit click is the only agent-starting action.
- Do not require stash. Two-way sync with stash is out of scope for this
  tranche.
- Do not copy or archive agent transcripts.
- Do not send transcript content anywhere unless model judgment is enabled, and
  then only to the configured backend.
- Do not estimate completion times.
- Do not support team or multi-device views.
- Menubar integration waits until the menubar talks to the service.

## Acceptance Criteria

1. With model judgment disabled, a Codex and a Claude Code session each show a
   ledger with verbatim ask, a fallback item, an evidence-tagged trail grouped
   by turn, and correct state. No subprocess or network model call occurs.
2. A Codex desktop session appears with state derived from its turn events.
3. Non-zero exit codes, test summaries, commits, and PR URLs from fixtures
   appear as explicit evidence with correct values.
4. Claude `PermissionRequest` / `Notification(permission_prompt)` and Codex
   `PermissionRequest` hooks set `needs_input` within 10 seconds and raise one
   alert; without hooks only "possibly waiting" appears and no alert fires.
5. A finished turn enters `review`; accept, accept-with-gaps, and follow-up
   each produce the specified state and persist across restart.
6. Follow-up and correction prompts are generated without a model call and are
   only copied, never sent.
7. With deviation `conservative`, a fixture with a long unmatched run raises
   exactly one off-plan alert; with `off`, none.
8. With model judgment enabled, an item becomes `done` only if the judge cites
   an existing evidence id for that session; otherwise `unverified`.
9. A session dispatched from a todo uses that todo's checklist as its
   requirement items; an attribution suggestion, once confirmed, survives
   resume and restart.
10. Accepting the last outstanding checklist item prompts to mark the todo done;
    confirming advances the goal from k/n to k+1/n.
11. A goal with no accepted todo, linked activity, or edit for 7 days is
    flagged as stale in the goals view.
12. Ledger rows older than the retention window are deleted by the retention
    job.
13. Every toggle in behavior 25 is readable and writable through config and the
    settings API, and each one demonstrably changes behavior in a test.
14. With no stash installed and no external upserts, a user can create a goal,
    add todos with checklists, dispatch or attribute sessions, accept work, and
    see goal progress, entirely inside Keepline.
