import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase, getDatabase, runSql } from '../infrastructure/database/sqlite.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';
import { setupUser } from '../services/auth.service.js';
import {
  beginSessionReconciliation,
  invalidateSessionClaims,
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
    runSql("DELETE FROM metadata WHERE key = 'session_reconciliation'");
    closeDatabase();
  });

  test.each([false, true])('rejects recovery after peer invalidation (owner dead: %s)', async (ownerDead) => {
    sessionRepository.upsert({
      sessionId: 'gate-recovery-session',
      directory: '/tmp/gate-recovery',
      status: 'lost',
      title: 'Interrupted',
      initialPrompt: 'Prompt',
      lastActiveAt: new Date(),
    });
    const ownerToken = beginSessionReconciliation('daemon');
    invalidateSessionClaims(ownerToken);
    if (ownerDead) {
      runSql(`UPDATE metadata SET value = json_set(value, '$.pid', ?)
        WHERE key = 'session_reconciliation'`, [2_147_483_646]);
    }
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

  test('serves static requests without acquiring the shared SQLite write lock', async () => {
    const server = await startWebServer(0, {
      serviceURL: 'http://127.0.0.1:3377',
      serviceProbe: async () => Response.json({
        success: true, data: { status: 'ok', mode: 'service', scan: { completed: true } },
      }),
    });
    const db = getDatabase();
    db.exec('PRAGMA busy_timeout = 0');
    const child = Bun.spawn([process.execPath, '-e', `
      import { getDatabase } from './src/infrastructure/database/sqlite.ts';
      const db = getDatabase(); db.exec('BEGIN IMMEDIATE');
      console.log('write-lock-held'); await Bun.sleep(350); db.exec('COMMIT');
    `], { env: process.env, stdout: 'pipe', stderr: 'pipe' });
    try {
      const output = await child.stdout.getReader().read();
      expect(new TextDecoder().decode(output.value)).toContain('write-lock-held');
      const response = await fetch(`http://127.0.0.1:${server.port}/assets/absent-gate-fixture.js`, {
        headers: { host: '127.0.0.1:0' },
      });
      expect(response.status).toBe(404);
    } finally {
      if (child.exitCode === null) child.kill('SIGKILL');
      await child.exited;
      db.exec('PRAGMA busy_timeout = 5000');
      server.stop(true);
    }
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
