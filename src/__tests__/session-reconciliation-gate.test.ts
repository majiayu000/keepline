import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
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
