/**
 * Regression tests for GH #75: the hook server must parse Claude Code's native
 * stdin payload (`hook_event_name`, `tool_response`, ...) — not the old
 * `event_type` shape that depended on non-existent `$CLAUDE_*` env vars.
 */

import { describe, expect, test } from 'bun:test';
import { parseHookEvent } from '../adapters/hook/server.js';

const SESSION = 'session-abcdef01';

describe('parseHookEvent (GH #75)', () => {
  test('parses a native PostToolUse payload', () => {
    const event = parseHookEvent({
      hook_event_name: 'PostToolUse',
      session_id: SESSION,
      cwd: '/tmp/repo',
      tool_name: 'Edit',
      tool_input: { file_path: '/tmp/repo/a.ts' },
      tool_response: { content: 'ok' },
    });

    expect(event).not.toBeNull();
    expect(event?.event_type).toBe('PostToolUse');
    expect(event?.session_id).toBe(SESSION);
    expect((event as { tool_name: string }).tool_name).toBe('Edit');
    // tool output is normalized from tool_response and stringified.
    expect((event as { tool_output?: string }).tool_output).toBe('{"content":"ok"}');
    // timestamp is stamped on receipt (Claude does not send one).
    expect(typeof event?.timestamp).toBe('string');
  });

  test('parses a native UserPromptSubmit payload', () => {
    const event = parseHookEvent({
      hook_event_name: 'UserPromptSubmit',
      session_id: SESSION,
      cwd: '/tmp/repo',
      prompt: 'do the thing',
    });

    expect(event?.event_type).toBe('UserPromptSubmit');
    expect((event as { prompt: string }).prompt).toBe('do the thing');
  });

  test('parses a native Stop payload', () => {
    const event = parseHookEvent({
      hook_event_name: 'Stop',
      session_id: SESSION,
      stop_reason: 'completed',
    });

    expect(event?.event_type).toBe('Stop');
    expect((event as { reason?: string }).reason).toBe('completed');
  });

  test('rejects the legacy event_type shape (no hook_event_name)', () => {
    expect(
      parseHookEvent({
        event_type: 'PostToolUse',
        session_id: SESSION,
        cwd: '/tmp/repo',
        timestamp: '2026-01-01T00:00:00Z',
        tool_name: 'Edit',
        tool_input: {},
      })
    ).toBeNull();
  });

  test('rejects unknown event types, bad session ids and non-objects', () => {
    expect(parseHookEvent({ hook_event_name: 'Bogus', session_id: SESSION })).toBeNull();
    expect(parseHookEvent({ hook_event_name: 'Stop', session_id: "x' OR '1'='1" })).toBeNull();
    expect(parseHookEvent(null)).toBeNull();
    expect(parseHookEvent('not-an-object')).toBeNull();
  });

  test('rejects a tool event with no tool_name', () => {
    expect(
      parseHookEvent({ hook_event_name: 'PostToolUse', session_id: SESSION, tool_input: {} })
    ).toBeNull();
  });
});
