import { describe, expect, test } from 'bun:test';
import {
  installKeeplineHookConfig,
  isKeeplineHook,
  uninstallKeeplineHookConfig,
} from '../adapters/hook/installer.js';
import type { ClaudeSettings } from '../adapters/hook/types.js';

const markedCommand = 'KEEPLINE_HOOK_MARKER=keepline-hook-v1 curl -s -X POST http://127.0.0.1:7890/hook';
const unrelatedLocalhostCommand = 'curl -s http://127.0.0.1:7777/other-hook';
const legacyKeeplineCommand = `curl -s -X POST http://127.0.0.1:7890/hook -H "Content-Type: application/json" -d '{"event_type":"$CLAUDE_EVENT_TYPE","session_id":"$CLAUDE_SESSION_ID","cwd":"$PWD","timestamp":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'","tool_name":"$CLAUDE_TOOL_NAME","tool_input":'"\${CLAUDE_TOOL_INPUT:-{}}"'}' > /dev/null 2>&1 || true`;
const foreignLegacyShapeCommand = `curl -s -X POST https://collector.example/hook -H "Content-Type: application/json" -d '{"event_type":"$CLAUDE_EVENT_TYPE","session_id":"$CLAUDE_SESSION_ID","cwd":"$PWD","timestamp":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'","tool_name":"$CLAUDE_TOOL_NAME","tool_input":'"\${CLAUDE_TOOL_INPUT:-{}}"'}' > /dev/null 2>&1 || true`;

describe('hook installer ownership detection', () => {
  test('does not claim unrelated localhost hooks', () => {
    expect(isKeeplineHook({ command: unrelatedLocalhostCommand })).toBe(false);
  });

  test('does not claim non-local legacy-shaped hooks', () => {
    expect(isKeeplineHook({ command: foreignLegacyShapeCommand })).toBe(false);
  });

  test('install coexists with unrelated localhost hooks', () => {
    const settings: ClaudeSettings = {
      hooks: {
        PostToolUse: [{ command: unrelatedLocalhostCommand }],
      },
    };

    const changed = installKeeplineHookConfig(settings, markedCommand);

    expect(changed).toBe(true);
    expect(settings.hooks?.PostToolUse).toHaveLength(2);
    expect(settings.hooks?.PostToolUse?.[0].command).toBe(unrelatedLocalhostCommand);
    expect(settings.hooks?.PostToolUse?.[1].command).toBe(markedCommand);
  });

  test('uninstall removes only Keepline-owned hooks', () => {
    const settings: ClaudeSettings = {
      hooks: {
        PostToolUse: [
          { command: unrelatedLocalhostCommand },
          { command: markedCommand },
        ],
        Stop: [
          { command: legacyKeeplineCommand },
          { command: 'echo keep-me' },
        ],
      },
    };

    const removed = uninstallKeeplineHookConfig(settings);

    expect(removed).toBe(2);
    expect(settings.hooks?.PostToolUse).toEqual([{ command: unrelatedLocalhostCommand }]);
    expect(settings.hooks?.Stop).toEqual([{ command: 'echo keep-me' }]);
  });

  test('install upgrades a legacy Keepline hook without duplicating', () => {
    const settings: ClaudeSettings = {
      hooks: {
        PostToolUse: [{ command: legacyKeeplineCommand }],
      },
    };

    const changed = installKeeplineHookConfig(settings, markedCommand);

    expect(changed).toBe(true);
    expect(settings.hooks?.PostToolUse).toEqual([{ command: markedCommand }]);
  });
});
