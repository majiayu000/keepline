# Cursor Runtime Discovery Spec

Parent issue: https://github.com/majiayu000/claude-hub/issues/44
Issue: https://github.com/majiayu000/claude-hub/issues/52

## Summary

Reserve `runtimeId: "cursor"` in the runtime-neutral adapter model, but do not implement an automatic Cursor session-history adapter from Cursor's private on-disk state yet.

The first safe Cursor integration should come from one of these explicit source contracts:

1. A Cursor-published stable local/session API or CLI export.
2. A Keepline-owned Cursor/VS Code compatible extension that emits versioned runtime session events.
3. A user-configured export or log path with a documented schema version and opt-in scope.

Until one of those exists, Cursor support should stay at discovery/spec level. Internal IDE state, SQLite files, or reverse-engineered workspace storage keys are research evidence only, not product source truth.

## Source Truth Matrix

| Candidate source | Decision | Reason |
|---|---|---|
| Cursor official API or CLI export | Accept when available | Stable contract can define runtime session IDs, cwd/project identity, activity timestamps, status, and recovery capabilities. |
| Keepline-owned Cursor extension | Accept for first implementation | Cursor is VS Code-family enough that an extension can use the documented VS Code extension storage, command, output, progress, and workspace APIs. Keepline owns the event schema instead of parsing Cursor internals. |
| User-configured Cursor export/log path | Accept with opt-in | Safe only when the user chooses the path and the file format has a versioned schema. |
| Cursor app `workspaceStorage` or `state.vscdb` | Reject for default adapter | This is private IDE/application state. Keys can change across Cursor versions and can contain user/editor state that is not a session contract. |
| Cursor UI state or window metadata | Reject | UI state is presentation, not durable runtime truth. |
| Heuristics over process names or window titles | Reject | Process/window hints can help live attribution after a session exists, but cannot create authoritative historical sessions. |

## Runtime Descriptor

Future descriptor shape:

```ts
const cursorDescriptor: RuntimeDescriptor = {
  id: 'cursor',
  displayName: 'Cursor',
  kind: 'ide',
  executableNames: ['Cursor'],
  sessionPathHints: [],
  capabilities: [],
}
```

Capability rules:

- Do not advertise `session-history` until the adapter has a stable event, export, or API source.
- Do not advertise `resume` unless Cursor exposes a structured command/API to reopen the exact runtime session.
- Do not advertise `quota`, `plans`, or `hooks` without matching feature provider contracts.
- Live process attribution may identify Cursor as an IDE process, but that does not create historical sessions by itself.

## Preferred Implementation Path

1. Add a small Cursor extension or extension-compatible bridge that writes newline-delimited JSON events to a user-configured Keepline path.
2. Define an event schema owned by Keepline, not Cursor internals:

```ts
type CursorRuntimeEvent =
  | {
      schemaVersion: 1
      type: 'session_started'
      runtimeSessionId: string
      workspaceRoot: string
      title?: string
      occurredAt: string
    }
  | {
      schemaVersion: 1
      type: 'activity'
      runtimeSessionId: string
      workspaceRoot: string
      summary?: string
      filesTouched?: string[]
      occurredAt: string
    }
  | {
      schemaVersion: 1
      type: 'session_completed'
      runtimeSessionId: string
      workspaceRoot: string
      outcome?: 'completed' | 'failed' | 'cancelled'
      occurredAt: string
    }
```

3. Parse those events into `RuntimeSession` records through `AgentRuntimeAdapter`.
4. Apply the same project identity rules as Claude Code and Codex: git root, normalized cwd fallback, Unknown only for empty or unnormalizable cwd.
5. Keep inferred progress as evidence or suggestions. Do not let extension events silently mark WorkItems done.

## Non-Goals

- Do not read Cursor account tokens, auth files, model prompts, or private application databases.
- Do not parse undocumented Cursor SQLite keys as a default source.
- Do not require users to disable privacy protections.
- Do not implement a Cursor adapter in issue #52.
- Do not treat VS Code storage APIs as proof of Cursor's private schema.

## Privacy and Safety

Cursor's public data-use page documents account/privacy behavior, including Privacy Mode and codebase indexing behavior, but it does not define a local session-history storage contract. Any Cursor adapter must therefore minimize local collection and require explicit user opt-in for extension/export paths.

The adapter must record per-runtime scan errors without hiding Claude Code or Codex sessions. Missing Cursor configuration is a no-op/debug state, not a warning or startup failure.

## Follow-Up Implementation Issue

Open a Cursor adapter implementation issue only after one source contract is selected. Use this template:

```md
## Goal

Implement CursorRuntimeAdapter from the approved Cursor source contract.

## Source Contract

- Source type: Cursor API | Keepline extension event log | user-configured export path
- Schema version:
- User opt-in path/config:
- Runtime session ID field:
- Project/cwd field:
- Activity timestamp field:

## Acceptance Criteria

- Missing Cursor source is no-op/debug.
- Bad Cursor event file reports RuntimeScanResult.errors without hiding other runtimes.
- RuntimeSession.runtimeId is `cursor`.
- Project filtering uses resolved projectRoot, not raw cwd text.
- No undocumented Cursor application DB keys are parsed.
- Tests cover missing source, bad event, good event, and projectRoot filtering.
```

## References

- Cursor Data Use & Privacy Overview: https://cursor.com/data-use
- VS Code extension common capabilities and storage APIs: https://code.visualstudio.com/api/extension-capabilities/common-capabilities
- VS Code API reference: https://code.visualstudio.com/api/references/vscode-api
