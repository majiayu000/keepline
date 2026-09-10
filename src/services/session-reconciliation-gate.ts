/**
 * Cross-process gate for startup/full session reconciliation.
 *
 * Daemon, Service Mode, and the standalone dashboard share SQLite. While one
 * owner invalidates live claims and rescans, peer dashboards must reject
 * recovery so a still-running agent cannot be duplicated.
 *
 * Completion is owner-token aware: only the current acquisition may clear the
 * gate. Failed reconciliations leave a durable `failed` state so peers keep
 * rejecting recovery after the owner process exits.
 */

import { randomUUID } from 'crypto';
import { queryOne, runSql } from '../infrastructure/database/sqlite.js';

const METADATA_KEY = 'session_reconciliation';

export type SessionReconciliationOwner = 'daemon' | 'service' | 'web';

export type SessionReconciliationToken = string;

interface SessionReconciliationState {
  status: 'running' | 'ready' | 'failed';
  owner?: SessionReconciliationOwner;
  token?: SessionReconciliationToken;
  pid?: number;
  startedAt?: string;
  completedAt?: string;
  failedAt?: string;
  error?: string;
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

/** Mark shared session state as under reconciliation. Returns an owner token. */
export function beginSessionReconciliation(
  owner: SessionReconciliationOwner
): SessionReconciliationToken {
  const token = randomUUID();
  writeState({
    status: 'running',
    owner,
    token,
    pid: process.pid,
    startedAt: new Date().toISOString(),
  });
  return token;
}

/**
 * Clear the shared gate after live claims are restored.
 * Only the current owner token may transition running → ready.
 */
export function completeSessionReconciliation(
  token?: SessionReconciliationToken
): boolean {
  const state = readState();
  if (!state || state.status !== 'running') return false;
  if (token != null && state.token != null && state.token !== token) {
    return false;
  }
  writeState({
    status: 'ready',
    completedAt: new Date().toISOString(),
  });
  return true;
}

/**
 * Persist a failed reconciliation so peer recovery stays blocked even after
 * the owner process exits. Only the current owner token may write failure.
 */
export function failSessionReconciliation(
  token?: SessionReconciliationToken,
  error?: string
): boolean {
  const state = readState();
  if (!state || state.status !== 'running') return false;
  if (token != null && state.token != null && state.token !== token) {
    return false;
  }
  writeState({
    status: 'failed',
    owner: state.owner,
    token: state.token,
    pid: state.pid,
    startedAt: state.startedAt,
    failedAt: new Date().toISOString(),
    error,
  });
  return true;
}

/**
 * True while another Keepline process is mid invalidate-and-full-scan, or after
 * a failed reconciliation that has not yet been superseded by a successful one.
 * Stale `running` locks from crashed owners (without a durable failure) clear.
 */
export function isSessionReconciliationRunning(): boolean {
  const state = readState();
  if (!state) return false;
  if (state.status === 'failed') return true;
  if (state.status !== 'running') return false;
  if (typeof state.pid === 'number' && state.pid !== process.pid && !processExists(state.pid)) {
    return false;
  }
  return true;
}
