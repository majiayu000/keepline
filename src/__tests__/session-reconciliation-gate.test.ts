import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase, runSql } from '../infrastructure/database/sqlite.js';
import {
  beginSessionReconciliation,
  completeSessionReconciliation,
  failSessionReconciliation,
  isSessionReconciliationRunning,
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

  test('ignores stale locks from dead owner pids', () => {
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
