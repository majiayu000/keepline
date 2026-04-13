import { describe, expect, test } from 'bun:test';
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
});
