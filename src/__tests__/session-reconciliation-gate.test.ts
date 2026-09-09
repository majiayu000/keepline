import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase, runSql } from '../infrastructure/database/sqlite.js';
import {
  beginSessionReconciliation,
  completeSessionReconciliation,
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
    beginSessionReconciliation('daemon');
    expect(isSessionReconciliationRunning()).toBe(true);
    completeSessionReconciliation();
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
        pid: 2_147_483_646,
        startedAt: new Date().toISOString(),
      })]
    );
    expect(isSessionReconciliationRunning()).toBe(false);
  });
});
