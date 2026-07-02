import { afterEach, describe, expect, test } from 'bun:test';
import type { FastifyInstance } from 'fastify';
import {
  createHookServer,
  isValidHookEvent,
  normalizeHookEvent,
} from '../adapters/hook/server.js';

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

describe('hook server request security', () => {
  let server: FastifyInstance | null = null;

  afterEach(async () => {
    if (server) {
      await server.close();
      server = null;
    }
  });

  function app(): FastifyInstance {
    server = createHookServer();
    return server;
  }

  test('rejects non-loopback Host headers before hook validation', async () => {
    const response = await app().inject({
      method: 'POST',
      url: '/hook',
      headers: {
        host: 'attacker.example',
        'content-type': 'application/json',
      },
      payload: {},
    });

    expect(response.statusCode).toBe(403);
    const body = response.json() as { success: boolean; error: string };
    expect(body).toEqual({ success: false, error: 'Forbidden' });
  });

  test('rejects cross-origin context reads', async () => {
    const response = await app().inject({
      method: 'GET',
      url: '/context?path=/tmp/project',
      headers: {
        host: '127.0.0.1:7890',
        origin: 'https://attacker.example',
      },
    });

    expect(response.statusCode).toBe(403);
  });

  test('accepts loopback health requests', async () => {
    const response = await app().inject({
      method: 'GET',
      url: '/health',
      headers: {
        host: 'localhost:7890',
      },
    });

    expect(response.statusCode).toBe(200);
    expect(response.json()).toMatchObject({ status: 'ok' });
  });

  test('allows loopback hook requests to reach payload validation', async () => {
    const response = await app().inject({
      method: 'POST',
      url: '/hook',
      headers: {
        host: '127.0.0.1:7890',
        'content-type': 'application/json',
      },
      payload: {},
    });

    expect(response.statusCode).toBe(400);
    const body = response.json() as { success: boolean; error: string };
    expect(body).toEqual({
      success: false,
      error: 'Invalid hook event payload',
    });
  });

  test('rejects cross-site browser fetch metadata', async () => {
    const response = await app().inject({
      method: 'POST',
      url: '/hook',
      headers: {
        host: '127.0.0.1:7890',
        'sec-fetch-site': 'cross-site',
        'content-type': 'application/json',
      },
      payload: {},
    });

    expect(response.statusCode).toBe(403);
  });
});
