import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import sessions from '../web/api/routes/sessions.js';
import { setupUser } from '../services/auth.service.js';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';
import { sessionRepo } from '../db/index.js';

describe('Basic Sessions Route Contract', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('fields=basic returns lightweight session payloads without usageStats', async () => {
    sessionRepo.upsert({
      sessionId: 'session-basic-1',
      directory: '/tmp/claude-hub-basic-contract',
      status: 'running',
      title: 'Investigate hot loop',
      initialPrompt: 'Profile the parser',
      lastTool: 'Edit',
      lastToolInput: JSON.stringify({ path: '/tmp/claude-hub-basic-contract/src/parser.ts' }),
      currentFile: '/tmp/claude-hub-basic-contract/src/parser.ts',
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
      directory: '/tmp/claude-hub-basic-contract',
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
});
