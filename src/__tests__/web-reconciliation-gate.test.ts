import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';
import { setupUser } from '../services/auth.service.js';
import {
  beginSessionReconciliation,
  completeSessionReconciliation,
} from '../services/session-reconciliation-gate.js';
import { startWebServer } from '../web/api/server.js';
import recovery from '../web/api/routes/recovery.js';
import { setWebSessionSource } from '../web/api/session-source.js';

describe('web recovery reconciliation gate', () => {
  beforeEach(() => {
    resetDatabase();
    setWebSessionSource('standalone');
  });

  afterEach(() => {
    completeSessionReconciliation();
    closeDatabase();
  });

  test('rejects recovery while a peer owner is reconciling', async () => {
    sessionRepository.upsert({
      sessionId: 'gate-recovery-session',
      directory: '/tmp/gate-recovery',
      status: 'lost',
      title: 'Interrupted',
      initialPrompt: 'Prompt',
      lastActiveAt: new Date(),
    });
    beginSessionReconciliation('daemon');
    const { token } = await setupUser('gate-recovery-user', 'password123');

    const response = await recovery.fetch(new Request(
      'http://localhost/gate-recovery-session/recover',
      {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        body: JSON.stringify({ method: 'resume', openTerminal: false }),
      }
    ));

    expect(response.status).toBe(503);
    expect(await response.json()).toEqual({
      success: false,
      error: 'Startup reconciliation is still running',
    });
  });
});

describe('standalone web server bind-before-invalidate', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('does not invalidate live claims when the dashboard port is occupied', async () => {
    sessionRepository.upsert({
      sessionId: 'same-port-running',
      directory: '/tmp/repo',
      status: 'running',
      pid: 4242,
      tty: 'ttys009',
    });

    const occupied = Bun.serve({
      hostname: '127.0.0.1',
      port: 0,
      fetch: () => new Response('occupied'),
    });

    try {
      // Same port as the probe URL → hasCompatibleService short-circuits to false,
      // then Bun.serve must fail before markActiveSessionsInterrupted runs.
      await expect(startWebServer(occupied.port!, {
        serviceURL: `http://127.0.0.1:${occupied.port}`,
      })).rejects.toThrow();

      const persisted = sessionRepository.findBySessionId('same-port-running');
      expect(persisted?.status).toBe('running');
      expect(persisted?.pid).toBe(4242);
      expect(persisted?.tty).toBe('ttys009');
    } finally {
      occupied.stop(true);
    }
  });
});
