import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { chmodSync, mkdtempSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase, execSql, queryOne, runSql } from '../infrastructure/database/sqlite.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';
import {
  beginSessionReconciliation,
  completeSessionReconciliation,
  failSessionReconciliation,
  isSessionReconciliationRunning,
  invalidateSessionClaims,
} from '../services/session-reconciliation-gate.js';

describe('session reconciliation gate', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('reports running only between begin and complete', () => {
    expect(isSessionReconciliationRunning()).toBe(false);
    const token = beginSessionReconciliation('daemon');
    expect(isSessionReconciliationRunning()).toBe(true);
    expect(completeSessionReconciliation(token)).toBe(true);
    expect(isSessionReconciliationRunning()).toBe(false);
  });

  test('ignores stale locks from dead owner pids before invalidation', () => {
    beginSessionReconciliation('daemon');
    runSql(
      `INSERT OR REPLACE INTO metadata (key, value, updated_at)
       VALUES ('session_reconciliation', ?, datetime('now'))`,
      [JSON.stringify({
        status: 'running',
        owner: 'daemon',
        token: 'stale-token',
        pid: 2_147_483_646,
        startedAt: new Date().toISOString(),
      })]
    );
    expect(isSessionReconciliationRunning()).toBe(false);
  });

  test('keeps recovery blocked when Service Mode is killed after invalidation', async () => {
    sessionRepository.upsert({
      sessionId: 'killed-owner-live-session',
      directory: '/tmp/killed-owner',
      status: 'running',
      pid: process.pid,
      tty: 'ttys001',
    });
    const child = Bun.spawn([process.execPath, '-e', `
      import { startKeeplineService } from './src/services/service-runtime.ts';
      await startKeeplineService({
        port: 0, hookPort: 0, scanIntervalMs: 0,
        scanCommand: [process.execPath, '-e', 'setInterval(() => {}, 1000)'],
      });
      console.log('invalidated');
    `], { env: process.env, stdout: 'pipe', stderr: 'ignore' });
    try {
      const reader = child.stdout.getReader();
      let output = '';
      while (!output.includes('invalidated')) {
        const chunk = await reader.read();
        if (chunk.done) break;
        output += new TextDecoder().decode(chunk.value);
      }
      reader.releaseLock();
      expect(output).toContain('invalidated');
      child.kill('SIGKILL');
      await child.exited;
      expect(sessionRepository.findBySessionId('killed-owner-live-session')?.status).toBe('lost');
      expect(sessionRepository.findBySessionId('killed-owner-live-session')?.pid).toBeUndefined();
      expect(isSessionReconciliationRunning()).toBe(true);
      const state = JSON.parse(queryOne<{ value: string }>(
        "SELECT value FROM metadata WHERE key = 'session_reconciliation'"
      )!.value);
      expect(state.status).toBe('failed');
      closeDatabase();
      expect(isSessionReconciliationRunning()).toBe(true);
    } finally {
      if (child.exitCode === null) child.kill('SIGKILL');
      await child.exited;
    }
  });

  test('omitted and empty owner tokens cannot complete or fail another owner gate', () => {
    const token = beginSessionReconciliation('daemon');
    const complete = completeSessionReconciliation as (token?: string) => boolean;
    const fail = failSessionReconciliation as (token?: string) => boolean;
    expect(complete()).toBe(false);
    expect(complete('')).toBe(false);
    expect(fail()).toBe(false);
    expect(fail('')).toBe(false);
    expect(isSessionReconciliationRunning()).toBe(true);
    expect(completeSessionReconciliation(token)).toBe(true);
  });

  test('a replacement owner killed before invalidation cannot drop a previous failure', () => {
    const first = beginSessionReconciliation('daemon');
    failSessionReconciliation(first, 'full scan failed');
    beginSessionReconciliation('web');
    const state = JSON.parse(queryOne<{ value: string }>(
      "SELECT value FROM metadata WHERE key = 'session_reconciliation'"
    )!.value);
    runSql("UPDATE metadata SET value = ? WHERE key = 'session_reconciliation'", [
      JSON.stringify({ ...state, pid: 2_147_483_646 }),
    ]);
    expect(isSessionReconciliationRunning()).toBe(true);
  });

  test('rolls back the invalidated phase when clearing live claims fails', () => {
    sessionRepository.upsert({
      sessionId: 'invalidation-rollback', directory: '/tmp/rollback',
      status: 'running', pid: process.pid, tty: 'ttys002',
    });
    const token = beginSessionReconciliation('daemon');
    execSql(`CREATE TRIGGER reject_invalidation BEFORE UPDATE ON sessions
      BEGIN SELECT RAISE(ABORT, 'fixture invalidation failed'); END`);
    try {
      expect(() => invalidateSessionClaims(token)).toThrow('fixture invalidation failed');
      expect(sessionRepository.findBySessionId('invalidation-rollback')?.pid).toBe(process.pid);
      const state = JSON.parse(queryOne<{ value: string }>(
        "SELECT value FROM metadata WHERE key = 'session_reconciliation'"
      )!.value);
      expect(state.status).toBe('running');
    } finally {
      execSql('DROP TRIGGER reject_invalidation');
    }
  });

  test('a stale owner cannot invalidate live claims for a newer owner', () => {
    sessionRepository.upsert({
      sessionId: 'stale-invalidation', directory: '/tmp/stale',
      status: 'running', pid: process.pid,
    });
    const first = beginSessionReconciliation('daemon');
    const second = beginSessionReconciliation('web');
    expect(() => invalidateSessionClaims(first)).toThrow('ownership changed');
    expect(sessionRepository.findBySessionId('stale-invalidation')?.status).toBe('running');
    expect(invalidateSessionClaims(second)).toBe(1);
    expect(sessionRepository.findBySessionId('stale-invalidation')?.status).toBe('lost');
    expect(completeSessionReconciliation(first)).toBe(false);
    expect(completeSessionReconciliation(second)).toBe(true);
  });

  test('waits for a concurrent writer before acquiring reconciliation ownership', async () => {
    beginSessionReconciliation('daemon');
    const child = Bun.spawn([process.execPath, '-e', `
      import { getDatabase } from './src/infrastructure/database/sqlite.ts';
      const db = getDatabase();
      db.exec('BEGIN IMMEDIATE');
      db.prepare("UPDATE metadata SET updated_at = datetime('now') WHERE key = ?")
        .run('session_reconciliation');
      console.log('write-lock-held');
      await Bun.sleep(350);
      db.exec('COMMIT');
    `], { env: process.env, stdout: 'pipe', stderr: 'pipe' });
    try {
      const output = await child.stdout.getReader().read();
      expect(new TextDecoder().decode(output.value)).toContain('write-lock-held');
      const token = beginSessionReconciliation('web');
      expect(completeSessionReconciliation(token)).toBe(true);
      expect(await child.exited).toBe(0);
    } finally {
      if (child.exitCode === null) child.kill('SIGKILL');
      await child.exited;
    }
  });

  test.each(['invalidated', 'failed'])('preserves another live process owner in %s state', async (status) => {
    const child = Bun.spawn([process.execPath, '-e', `
      import {
        beginSessionReconciliation, invalidateSessionClaims,
        failSessionReconciliation, completeSessionReconciliation,
      } from './src/services/session-reconciliation-gate.ts';
      const token = beginSessionReconciliation('daemon');
      invalidateSessionClaims(token);
      if (${JSON.stringify(status)} === 'failed') failSessionReconciliation(token, 'retrying');
      console.log('owner-acquired');
      await Bun.sleep(350);
      if (!completeSessionReconciliation(token)) process.exit(1);
    `], { env: process.env, stdout: 'pipe', stderr: 'pipe' });
    try {
      const output = await child.stdout.getReader().read();
      expect(new TextDecoder().decode(output.value)).toContain('owner-acquired');
      expect(() => beginSessionReconciliation('web')).toThrow('live process');
      expect(isSessionReconciliationRunning()).toBe(true);
      expect(await child.exited).toBe(0);
      expect(isSessionReconciliationRunning()).toBe(false);
    } finally {
      if (child.exitCode === null) child.kill('SIGKILL');
      await child.exited;
    }
  });

  test.each(['invalidated', 'failed'])('allows takeover after an owner PID is reused in %s state', async (status) => {
    const token = beginSessionReconciliation('daemon');
    invalidateSessionClaims(token);
    if (status === 'failed') failSessionReconciliation(token, 'failed before exit');
    const unrelated = Bun.spawn([process.execPath, '-e', `
      console.log('unrelated-process'); setInterval(() => {}, 1000);
    `], { env: process.env, stdout: 'pipe', stderr: 'pipe' });
    try {
      await unrelated.stdout.getReader().read();
      runSql(`UPDATE metadata SET value = json_set(value,
        '$.pid', ?, '$.processStartedAt', 'Sat Jan 1 00:00:00 2000')
        WHERE key = 'session_reconciliation'`, [unrelated.pid]);
      const replacement = beginSessionReconciliation('web');
      expect(isSessionReconciliationRunning()).toBe(true);
      expect(completeSessionReconciliation(replacement)).toBe(true);
    } finally {
      unrelated.kill('SIGKILL');
      await unrelated.exited;
    }
  });

  test('resolves process identity through PATH', async () => {
    const directory = mkdtempSync(join(tmpdir(), 'keepline-identity-path-'));
    const ps = join(directory, 'ps');
    writeFileSync(ps, "#!/bin/sh\nprintf '%s\\n' 'fixture process start from PATH'\n");
    chmodSync(ps, 0o755);
    const child = Bun.spawn([process.execPath, '-e', `
      import { beginSessionReconciliation, completeSessionReconciliation }
        from './src/services/session-reconciliation-gate.ts';
      import { queryOne } from './src/infrastructure/database/sqlite.ts';
      const token = beginSessionReconciliation('web');
      const state = JSON.parse(queryOne("SELECT value FROM metadata WHERE key = 'session_reconciliation'").value);
      console.log(state.processStartedAt);
      completeSessionReconciliation(token);
    `], {
      env: { ...process.env, PATH: directory + ':' + process.env.PATH },
      stdout: 'pipe', stderr: 'pipe',
    });
    expect(await child.exited).toBe(0);
    expect((await new Response(child.stdout).text()).trim()).toBe('fixture process start from PATH');
  });

  test('only the current owner token may complete reconciliation', () => {
    const first = beginSessionReconciliation('daemon');
    const second = beginSessionReconciliation('service');
    expect(completeSessionReconciliation(first)).toBe(false);
    expect(isSessionReconciliationRunning()).toBe(true);
    expect(completeSessionReconciliation(second)).toBe(true);
    expect(isSessionReconciliationRunning()).toBe(false);
  });

  test('failed reconciliation keeps peers blocked after the owner exits', () => {
    const token = beginSessionReconciliation('daemon');
    expect(failSessionReconciliation(token, 'full scan failed')).toBe(true);
    expect(isSessionReconciliationRunning()).toBe(true);

    // Simulate a later peer observing the durable failure without a live owner pid.
    runSql(
      `INSERT OR REPLACE INTO metadata (key, value, updated_at)
       VALUES ('session_reconciliation', ?, datetime('now'))`,
      [JSON.stringify({
        status: 'failed',
        owner: 'daemon',
        token,
        pid: 2_147_483_646,
        failedAt: new Date().toISOString(),
        error: 'full scan failed',
      })]
    );
    expect(isSessionReconciliationRunning()).toBe(true);

    const recovery = beginSessionReconciliation('web');
    expect(completeSessionReconciliation(recovery)).toBe(true);
    expect(isSessionReconciliationRunning()).toBe(false);
  });

  test('stale owner cannot fail a newer owner gate', () => {
    const first = beginSessionReconciliation('daemon');
    const second = beginSessionReconciliation('web');
    expect(failSessionReconciliation(first, 'stale')).toBe(false);
    expect(isSessionReconciliationRunning()).toBe(true);
    expect(completeSessionReconciliation(second)).toBe(true);
  });
});
