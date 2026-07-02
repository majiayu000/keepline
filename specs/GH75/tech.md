# Hook Pipeline Recovery Tech Spec

Product spec: specs/GH75/product.md

Issue: https://github.com/majiayu000/keepline/issues/75

## External Contract

Claude Code command hooks receive the event JSON on stdin. Common fields include `session_id`, `transcript_path`, `cwd`, and `hook_event_name`. Tool events add `tool_name` and `tool_input`; `PostToolUse` adds `tool_response`; `UserPromptSubmit` adds `prompt`; `Stop` adds stop-specific fields.

The hook settings schema is event keyed and contains matcher groups with nested hook handlers:

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "..."
          }
        ]
      }
    ]
  }
}
```

## Implementation Plan

### Installer

Update `src/adapters/hook/installer.ts`:

- Generate one command that forwards stdin:

  ```bash
  KEEPLINE_HOOK_MARKER=keepline-hook-v1 curl -fsS -X POST http://127.0.0.1:<port>/hook -H "Content-Type: application/json" --data-binary @- > /dev/null 2>&1 || true
  ```

- Register Keepline hook handlers for:
  - `PreToolUse`
  - `PostToolUse`
  - `Notification`
  - `Stop`
  - `UserPromptSubmit`
- Use `matcher: "*"` only for tool events.
- Preserve unrelated hook handlers and remove/upgrade only Keepline-owned legacy hooks.
- Treat fully installed five-event config as installed; partial `PostToolUse` only should be repaired by reinstall.

### Server Boundary

Update `src/adapters/hook/server.ts`:

- Add a pure `normalizeHookEvent(raw, now)` boundary function.
- Accept both official `hook_event_name` and legacy `event_type`.
- Fill missing `timestamp` at receive time for official payloads.
- Convert official `tool_response` to internal string `tool_output` with `JSON.stringify`.
- Keep existing internal handlers unchanged after normalization.
- Reject malformed payloads with 400.

### Types

Update `src/adapters/hook/types.ts`:

- Model current hook settings as matcher groups with nested command handlers.
- Keep legacy direct command entries in the union for ownership detection and uninstall.
- Keep internal `HookEvent` normalized around `event_type` and `timestamp`.

## Tests

- `src/__tests__/hook.installer.test.ts`
  - unrelated localhost hooks are preserved.
  - foreign legacy-shaped hooks are not claimed.
  - install writes all five events in current schema.
  - uninstall removes only Keepline-owned handlers.
  - legacy Keepline command is upgraded without duplication.
  - reinstall of the full config is a no-op.

- `src/__tests__/hook.server.test.ts`
  - official `PostToolUse` stdin payload normalizes successfully.
  - legacy `event_type` payload remains accepted.
  - official `UserPromptSubmit` without command-synthesized timestamp is valid.
  - malformed tool payload is rejected.

## Risk Notes

- The command keeps `|| true` to avoid blocking Claude Code when Keepline is not running. This preserves existing non-blocking behavior but means users need status/health tooling to discover delivery failures.
- GH79 should add Host/Origin/auth checks; this issue only restores event delivery.
