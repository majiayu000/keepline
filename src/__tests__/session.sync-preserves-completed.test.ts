/**
 * Regression test for GH #77: a session explicitly marked `completed` (by the
 * user in the dashboard or via a Stop hook) must not be resurrected to
 * lost/idle by a later sync. detectSessionStatus never returns 'completed',
 * so syncSessions has to preserve the persisted terminal status itself.
 */

import { describe, test, expect, beforeEach, afterEach, mock } from 'bun:test';

// Feed a single controlled Claude session through the scanner boundary, and
// report no live processes so detectSessionStatus(null, ...) computes 'lost'.
const COMPLETED_ID = 'completed-session-0001';

mock.module('../adapters/process/scanner.js', () => ({
  getCachedProcesses: () => [],
  clearProcessCache: () => {},
}));

mock.module('../adapters/claude/scanner.js', () => ({
  getAllSessionsWithFailures: async () => ({
    sessions: [
      {
        sessionId: COMPLETED_ID,
        client: 'claude',
        directory: '/tmp/repo',
        firstMessage: 'done task',
        messageCount: 3,
        toolCount: 2,
        lastActiveAt: new Date('2020-01-01T00:00:00.000Z'), // stale
      },
    ],
    failures: [],
  }),
}));

mock.module('../adapters/codex/scanner.js', () => ({
  getAllCodexSessionsWithFailures: async () => ({ sessions: [], failures: [] }),
}));

const { resetDatabase } = await import('../db/migrations.js');
const { closeDatabase } = await import('../infrastructure/database/sqlite.js');
const { sessionRepository } = await import(
  '../infrastructure/database/repositories/session.repository.js'
);
const { syncSessions } = await import('../services/session.service.js');

describe('syncSessions preserves completed status (GH #77)', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('a completed session is not reverted to lost by a later scan', async () => {
    sessionRepository.upsert({
      sessionId: COMPLETED_ID,
      client: 'claude',
      directory: '/tmp/repo',
      status: 'completed',
      title: 'Finished task',
      initialPrompt: 'done task',
      lastActiveAt: new Date('2020-01-01T00:00:00.000Z'),
      toolCount: 2,
      messageCount: 3,
    });

    await syncSessions();

    const after = sessionRepository.findBySessionId(COMPLETED_ID);
    expect(after?.status).toBe('completed');
  });

  test('a non-completed session with no process is still updated to lost', async () => {
    sessionRepository.upsert({
      sessionId: COMPLETED_ID,
      client: 'claude',
      directory: '/tmp/repo',
      status: 'running',
      title: 'Active task',
      initialPrompt: 'done task',
      lastActiveAt: new Date('2020-01-01T00:00:00.000Z'),
      toolCount: 2,
      messageCount: 3,
    });

    await syncSessions();

    const after = sessionRepository.findBySessionId(COMPLETED_ID);
    expect(after?.status).toBe('lost');
  });
});
