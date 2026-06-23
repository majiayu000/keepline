import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import sessions from '../web/api/routes/sessions.js';
import { setupUser } from '../services/auth.service.js';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';

describe('Basic Sessions Route Contract', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('fields=basic returns lightweight session payloads without usageStats', async () => {
    sessionRepository.upsert({
      sessionId: 'session-basic-1',
      directory: '/tmp/keepline-basic-contract',
      status: 'running',
      title: 'Investigate hot loop',
      initialPrompt: 'Profile the parser',
      lastTool: 'Edit',
      lastToolInput: JSON.stringify({ path: '/tmp/keepline-basic-contract/src/parser.ts' }),
      currentFile: '/tmp/keepline-basic-contract/src/parser.ts',
      lastMessage: 'Working on it',
      startedAt: new Date('2026-04-13T10:00:00.000Z'),
      lastActiveAt: new Date('2026-04-13T10:00:05.000Z'),
      toolCount: 4,
      messageCount: 2,
    });

    const { token } = await setupUser('basic-route-user', 'password123');
    const response = await sessions.fetch(new Request(
      'http://localhost/?skipSync=true&fields=basic',
      { headers: { Authorization: `Bearer ${token}` } }
    ));

    expect(response.status).toBe(200);

    const body = await response.json() as {
      success: boolean;
      data: {
        sessions: Array<Record<string, unknown>>;
        stats: { total: number; lost: number };
      };
    };

    expect(body.success).toBe(true);
    expect(body.data.stats.total).toBe(1);
    expect(body.data.stats.lost).toBe(1);
    expect(body.data.sessions).toHaveLength(1);

    const session = body.data.sessions[0];
    expect(session).toMatchObject({
      sessionId: 'session-basic-1',
      directory: '/tmp/keepline-basic-contract',
      title: 'Investigate hot loop',
      status: 'lost',
      toolCount: 4,
      messageCount: 2,
      processRunning: false,
    });
    expect(session.startedAt).toBe('2026-04-13T10:00:00.000Z');
    expect(session.lastActiveAt).toBe('2026-04-13T10:00:05.000Z');
    expect('usageStats' in session).toBe(false);
    expect('initialPrompt' in session).toBe(false);
    expect('lastMessage' in session).toBe(false);
    expect('lastTool' in session).toBe(false);
    expect('lastToolInput' in session).toBe(false);
  });

  test('details returns null usageStats when parsed source data is unavailable', async () => {
    sessionRepository.upsert({
      sessionId: 'session-no-source',
      directory: '/tmp/keepline-no-source',
      status: 'completed',
      title: 'No parsed source',
      initialPrompt: 'No source',
      lastActiveAt: new Date('2026-04-13T10:00:05.000Z'),
      toolCount: 0,
      messageCount: 1,
    });

    const { token } = await setupUser('details-route-user', 'password123');
    const response = await sessions.fetch(new Request(
      'http://localhost/session-no-source/details',
      { headers: { Authorization: `Bearer ${token}` } }
    ));

    expect(response.status).toBe(200);

    const body = await response.json() as {
      success: boolean;
      data: {
        usageStats: unknown;
      };
    };

    expect(body.success).toBe(true);
    expect(body.data.usageStats).toBeNull();
  });

  test('fields=basic search filters globally before pagination', async () => {
    sessionRepository.upsert({
      sessionId: 'session-search-match',
      directory: '/tmp/keepline-search',
      status: 'completed',
      title: 'Unrelated title',
      initialPrompt: 'Find the hidden global needle',
      lastActiveAt: new Date('2026-04-13T10:00:05.000Z'),
      toolCount: 0,
      messageCount: 1,
    });
    sessionRepository.upsert({
      sessionId: 'session-search-miss',
      directory: '/tmp/keepline-other',
      status: 'completed',
      title: 'Other work',
      initialPrompt: 'No matching text here',
      lastActiveAt: new Date('2026-04-13T10:00:06.000Z'),
      toolCount: 0,
      messageCount: 1,
    });

    const { token } = await setupUser('search-route-user', 'password123');
    const response = await sessions.fetch(new Request(
      'http://localhost/?skipSync=true&fields=basic&q=needle&limit=1',
      { headers: { Authorization: `Bearer ${token}` } }
    ));

    expect(response.status).toBe(200);

    const body = await response.json() as {
      success: boolean;
      data: {
        sessions: Array<{ sessionId: string }>;
        pagination: { total: number; hasMore: boolean };
      };
    };

    expect(body.success).toBe(true);
    expect(body.data.pagination.total).toBe(1);
    expect(body.data.pagination.hasMore).toBe(false);
    expect(body.data.sessions.map((session) => session.sessionId)).toEqual(['session-search-match']);
  });

  test('rejects invalid status filters instead of returning unfiltered sessions', async () => {
    sessionRepository.upsert({
      sessionId: 'session-invalid-status-filter',
      directory: '/tmp/keepline-invalid-status-filter',
      status: 'completed',
      title: 'Should not leak through invalid filter',
      initialPrompt: 'No source',
      lastActiveAt: new Date('2026-04-13T10:00:05.000Z'),
      toolCount: 0,
      messageCount: 1,
    });

    const { token } = await setupUser('invalid-status-route-user', 'password123');
    const response = await sessions.fetch(new Request(
      'http://localhost/?skipSync=true&fields=basic&status=runing',
      { headers: { Authorization: `Bearer ${token}` } }
    ));

    expect(response.status).toBe(400);

    const body = await response.json() as {
      success: boolean;
      error: string;
      data?: { sessions: Array<{ sessionId: string }> };
    };

    expect(body.success).toBe(false);
    expect(body.error).toContain('Invalid status filter: runing');
    expect(body.data).toBeUndefined();
  });
});
