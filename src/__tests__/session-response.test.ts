import { describe, expect, test } from 'bun:test';
import { generateTitle } from '../domain/session/entity.js';
import { serializeBasicSession } from '../web/api/session-response.js';

describe('Session Response Serialization', () => {
  test('serializeBasicSession emits only lightweight list fields', () => {
    const createdAt = new Date('2026-04-13T14:00:00.000Z');
    const updatedAt = new Date('2026-04-13T14:05:00.000Z');
    const startedAt = new Date('2026-04-13T13:55:00.000Z');
    const lastActiveAt = new Date('2026-04-13T14:04:00.000Z');

    const fullLikeSession = {
      id: 'session-row-1',
      sessionId: 'session-1',
      client: 'claude',
      runtimeId: 'claude-code',
      directory: '/tmp/project',
      status: 'running',
      title: 'Profile dashboard list path',
      initialPrompt: 'heavy prompt that should not leak',
      lastTool: 'Edit',
      lastToolInput: '{"path":"src/server.ts"}',
      currentFile: '/tmp/project/src/server.ts',
      lastMessage: 'heavy last message that should not leak',
      startedAt,
      lastActiveAt,
      completedAt: undefined,
      pid: 1234,
      tty: 'ttys001',
      toolCount: 7,
      messageCount: 3,
      createdAt,
      updatedAt,
      processRunning: true,
      cpuUsage: 1.5,
      memoryUsage: 2.5,
    };

    const serialized = serializeBasicSession(fullLikeSession as any);

    expect(serialized).toEqual({
      id: 'session-row-1',
      sessionId: 'session-1',
      client: 'claude',
      runtimeId: 'claude-code',
      directory: '/tmp/project',
      status: 'running',
      title: 'Profile dashboard list path',
      lastActiveAt: '2026-04-13T14:04:00.000Z',
      startedAt: '2026-04-13T13:55:00.000Z',
      completedAt: undefined,
      createdAt: '2026-04-13T14:00:00.000Z',
      updatedAt: '2026-04-13T14:05:00.000Z',
      pid: 1234,
      tty: 'ttys001',
      toolCount: 7,
      messageCount: 3,
      processRunning: true,
      cpuUsage: 1.5,
      memoryUsage: 2.5,
    });
    expect('initialPrompt' in serialized).toBe(false);
    expect('lastTool' in serialized).toBe(false);
    expect('lastToolInput' in serialized).toBe(false);
    expect('currentFile' in serialized).toBe(false);
    expect('lastMessage' in serialized).toBe(false);
  });

  test('generateTitle summarizes AGENTS instruction payloads by project', () => {
    const title = generateTitle(`# AGENTS.md instructions for /Users/me/project\n\n<INSTRUCTIONS>\nvery long payload`);

    expect(title).toBe('AGENTS.md: project');
  });

  test('generateTitle skips AGENTS preamble when task text follows', () => {
    const title = generateTitle(`# AGENTS.md instructions for /Users/me/project

<INSTRUCTIONS>
repo rules
</INSTRUCTIONS><environment_context>
cwd metadata
</environment_context>

Fix active issue queue`);

    expect(title).toBe('Fix active issue queue');
  });
});
