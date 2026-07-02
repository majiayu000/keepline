import { describe, expect, test } from 'bun:test';
import { isValidHookEvent, normalizeHookEvent } from '../adapters/hook/server.js';

const fixedNow = new Date('2026-07-02T15:30:00.000Z');

describe('hook server payload normalization', () => {
  test('accepts current Claude Code PostToolUse stdin payloads', () => {
    const event = normalizeHookEvent(
      {
        session_id: 'session-1234',
        transcript_path: '/tmp/transcript.jsonl',
        cwd: '/tmp/project',
        hook_event_name: 'PostToolUse',
        tool_name: 'Write',
        tool_input: {
          file_path: '/tmp/project/file.txt',
          content: 'hello',
        },
        tool_response: {
          filePath: '/tmp/project/file.txt',
          success: true,
        },
      },
      fixedNow
    );

    expect(event).toEqual({
      event_type: 'PostToolUse',
      session_id: 'session-1234',
      transcript_path: '/tmp/transcript.jsonl',
      cwd: '/tmp/project',
      timestamp: '2026-07-02T15:30:00.000Z',
      tool_name: 'Write',
      tool_input: {
        file_path: '/tmp/project/file.txt',
        content: 'hello',
      },
      tool_output: '{"filePath":"/tmp/project/file.txt","success":true}',
    });
  });

  test('accepts legacy Keepline event_type payloads', () => {
    const event = normalizeHookEvent({
      event_type: 'PreToolUse',
      session_id: 'session-1234',
      cwd: '/tmp/project',
      timestamp: '2026-07-02T15:31:00.000Z',
      tool_name: 'Bash',
      tool_input: {
        command: 'bun test',
      },
    });

    expect(event).toEqual({
      event_type: 'PreToolUse',
      session_id: 'session-1234',
      cwd: '/tmp/project',
      timestamp: '2026-07-02T15:31:00.000Z',
      transcript_path: undefined,
      tool_name: 'Bash',
      tool_input: {
        command: 'bun test',
      },
      tool_output: undefined,
    });
  });

  test('accepts UserPromptSubmit without a synthetic timestamp from the command', () => {
    expect(
      isValidHookEvent({
        session_id: 'session-1234',
        cwd: '/tmp/project',
        hook_event_name: 'UserPromptSubmit',
        prompt: 'Summarize this repo',
      })
    ).toBe(true);
  });

  test('rejects malformed tool payloads', () => {
    expect(
      normalizeHookEvent({
        session_id: 'session-1234',
        cwd: '/tmp/project',
        hook_event_name: 'PostToolUse',
        tool_name: 'Write',
      })
    ).toBeNull();
  });
});
