/**
 * Cross-process gate for startup/full session reconciliation.
 *
 * Daemon, Service Mode, and the standalone dashboard share SQLite. While one
 * owner invalidates live claims and rescans, peer dashboards must reject
 * recovery so a still-running agent cannot be duplicated.
 */

import { queryOne, runSql } from '../infrastructure/database/sqlite.js';

const METADATA_KEY = 'session_reconciliation';

export type SessionReconciliationOwner = 'daemon' | 'service' | 'web';

interface SessionReconciliationState {
  status: 'running' | 'ready';
  owner?: SessionReconciliationOwner;
  pid?: number;
  startedAt?: string;
  completedAt?: string;
}

function readState(): SessionReconciliationState | null {
  const row = queryOne<{ value: string }>(
    'SELECT value FROM metadata WHERE key = ?',
    [METADATA_KEY]
  );
  if (!row) return null;
  try {
    return JSON.parse(row.value) as SessionReconciliationState;
  } catch {
    return null;
  }
}

function writeState(state: SessionReconciliationState): void {
  runSql(
    `INSERT OR REPLACE INTO metadata (key, value, updated_at)
     VALUES (?, ?, datetime('now'))`,
    [METADATA_KEY, JSON.stringify(state)]
  );
}

function processExists(pid: number): boolean {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/** Mark shared session state as under reconciliation. */
export function beginSessionReconciliation(owner: SessionReconciliationOwner): void {
  writeState({
    status: 'running',
    owner,
    pid: process.pid,
    startedAt: new Date().toISOString(),
  });
}

/** Clear the shared reconciliation gate after live claims are restored. */
export function completeSessionReconciliation(): void {
  writeState({
    status: 'ready',
    completedAt: new Date().toISOString(),
  });
}

/**
 * True while another Keepline process is mid invalidate-and-full-scan.
 * Stale locks from crashed owners are treated as cleared.
 */
export function isSessionReconciliationRunning(): boolean {
  const state = readState();
  if (!state || state.status !== 'running') return false;
  if (typeof state.pid === 'number' && state.pid !== process.pid && !processExists(state.pid)) {
    return false;
  }
  return true;
}
