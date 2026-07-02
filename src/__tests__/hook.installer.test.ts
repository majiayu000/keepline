import { describe, expect, test } from 'bun:test';
import {
  installKeeplineHookConfig,
  isKeeplineHook,
  uninstallKeeplineHookConfig,
} from '../adapters/hook/installer.js';
import type { ClaudeSettings, ClaudeHookMatcher } from '../adapters/hook/types.js';

const markedCommand =
  "KEEPLINE_HOOK_MARKER=keepline-hook-v2 curl -s -X POST http://127.0.0.1:7890/hook -H 'Content-Type: application/json' --data-binary @- >/dev/null 2>&1 || true";
const unrelatedBlock: ClaudeHookMatcher = {
  hooks: [{ type: 'command', command: 'curl -s http://127.0.0.1:7777/other-hook' }],
};
const legacyKeeplineCommand = `curl -s -X POST http://127.0.0.1:7890/hook -H "Content-Type: application/json" -d '{"event_type":"$CLAUDE_EVENT_TYPE","session_id":"$CLAUDE_SESSION_ID","cwd":"$PWD","timestamp":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'","tool_name":"$CLAUDE_TOOL_NAME","tool_input":'"\${CLAUDE_TOOL_INPUT:-{}}"'}' > /dev/null 2>&1 || true`;

describe('hook installer ownership detection', () => {
  test('does not claim unrelated localhost hooks', () => {
    expect(
      isKeeplineHook({ type: 'command', command: 'curl -s http://127.0.0.1:7777/other-hook' })
    ).toBe(false);
  });

  test('does not claim non-local legacy-shaped hooks', () => {
    const foreign = legacyKeeplineCommand.replace('127.0.0.1:7890', 'collector.example');
    expect(isKeeplineHook({ type: 'command', command: foreign })).toBe(false);
  });

  test('registers under PostToolUse, Stop and UserPromptSubmit using the nested shape', () => {
    const settings: ClaudeSettings = {};

    const changed = installKeeplineHookConfig(settings, markedCommand);

    expect(changed).toBe(true);
    for (const event of ['PostToolUse', 'Stop', 'UserPromptSubmit'] as const) {
      const blocks = settings.hooks?.[event];
      expect(blocks).toHaveLength(1);
      const entry = blocks?.[0].hooks[0];
      expect(entry?.type).toBe('command');
      expect(entry?.command).toBe(markedCommand);
    }
  });

  test('install is idempotent (no duplicate registration)', () => {
    const settings: ClaudeSettings = {};
    installKeeplineHookConfig(settings, markedCommand);

    const changedAgain = installKeeplineHookConfig(settings, markedCommand);

    expect(changedAgain).toBe(false);
    expect(settings.hooks?.PostToolUse).toHaveLength(1);
  });

  test('install coexists with unrelated hooks in the same event', () => {
    const settings: ClaudeSettings = {
      hooks: { PostToolUse: [unrelatedBlock] },
    };

    const changed = installKeeplineHookConfig(settings, markedCommand);

    expect(changed).toBe(true);
    expect(settings.hooks?.PostToolUse).toHaveLength(2);
    expect(settings.hooks?.PostToolUse?.[0]).toEqual(unrelatedBlock);
    expect(settings.hooks?.PostToolUse?.[1].hooks[0].command).toBe(markedCommand);
  });

  test('uninstall removes only Keepline-owned hooks and drops empty blocks', () => {
    const settings: ClaudeSettings = {
      hooks: {
        PostToolUse: [
          unrelatedBlock,
          { hooks: [{ type: 'command', command: markedCommand }] },
        ],
        Stop: [
          {
            hooks: [
              { type: 'command', command: legacyKeeplineCommand },
              { type: 'command', command: 'echo keep-me' },
            ],
          },
        ],
      },
    };

    const removed = uninstallKeeplineHookConfig(settings);

    expect(removed).toBe(2);
    // Keepline-only block dropped; unrelated block preserved.
    expect(settings.hooks?.PostToolUse).toEqual([unrelatedBlock]);
    // Mixed block keeps the non-Keepline hook only.
    expect(settings.hooks?.Stop).toEqual([
      { hooks: [{ type: 'command', command: 'echo keep-me' }] },
    ]);
  });

  test('install upgrades a legacy Keepline hook in place without duplicating', () => {
    const settings: ClaudeSettings = {
      hooks: {
        PostToolUse: [{ hooks: [{ type: 'command', command: legacyKeeplineCommand }] }],
      },
    };

    const changed = installKeeplineHookConfig(settings, markedCommand);

    expect(changed).toBe(true);
    expect(settings.hooks?.PostToolUse).toHaveLength(1);
    expect(settings.hooks?.PostToolUse?.[0].hooks[0].command).toBe(markedCommand);
  });

  test('generated command forwards stdin and does not rely on $CLAUDE_* env vars', () => {
    const settings: ClaudeSettings = {};
    installKeeplineHookConfig(settings);
    const command = settings.hooks?.PostToolUse?.[0].hooks[0].command ?? '';

    expect(command).toContain('--data-binary @-');
    expect(command).not.toContain('$CLAUDE_EVENT_TYPE');
    expect(command).not.toContain('$CLAUDE_SESSION_ID');
    expect(command).not.toContain('$CLAUDE_TOOL_INPUT');
  });
});
