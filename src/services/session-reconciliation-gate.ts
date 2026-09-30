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
import { execFileSync } from 'child_process';
import { getDatabase, queryOne, runSql } from '../infrastructure/database/sqlite.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';

const METADATA_KEY = 'session_reconciliation';

export type SessionReconciliationOwner = 'daemon' | 'service' | 'web';

export type SessionReconciliationToken = string;

interface SessionReconciliationState {
  status: 'running' | 'invalidated' | 'ready' | 'failed';
  owner?: SessionReconciliationOwner;
  token?: SessionReconciliationToken;
  pid?: number;
  processStartedAt?: string;
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

function readProcessStartTime(pid: number): string | undefined {
  try {
    const startedAt = execFileSync('/bin/ps', ['-p', String(pid), '-o', 'lstart='], {
      encoding: 'utf8', timeout: 5_000,
      env: { ...process.env, LC_ALL: 'C', TZ: 'UTC' },
      stdio: ['ignore', 'pipe', 'pipe'],
    }).trim();
    if (!startedAt) throw new Error('Process start time is unavailable');
    return startedAt;
  } catch (error) {
    if (!processExists(pid)) return undefined;
    throw error;
  }
}

function ownerIsLive(state: SessionReconciliationState): boolean {
  return typeof state.pid === 'number' && !!state.processStartedAt &&
    readProcessStartTime(state.pid) === state.processStartedAt;
}

/** Mark shared session state as under reconciliation. Returns an owner token. */
export function beginSessionReconciliation(
  owner: SessionReconciliationOwner
): SessionReconciliationToken {
  const token = randomUUID();
  const processStartedAt = readProcessStartTime(process.pid);
  if (!processStartedAt) throw new Error('Reconciliation owner process identity is unavailable');
  getDatabase().transaction(() => {
    const previous = readState();
    // A process may reacquire its own gate; peers must let the live owner finish.
    if (previous && previous.status !== 'ready' &&
        typeof previous.pid === 'number' && previous.pid !== process.pid &&
        ownerIsLive(previous)) {
      throw new Error('Session reconciliation is already owned by another live process');
    }
    writeState({
      // Acquiring a new owner must not discard an unfinished invalidation.
      status: previous?.status === 'invalidated' || previous?.status === 'failed'
        ? 'invalidated' : 'running',
      owner,
      token,
      pid: process.pid,
      processStartedAt,
      startedAt: new Date().toISOString(),
    });
  }).immediate();
  return token;
}

/** Persist invalidation and clear live claims in the same SQLite transaction. */
export function invalidateSessionClaims(token: SessionReconciliationToken): number {
  return getDatabase().transaction(() => {
    const state = readState();
    if (!token || !state || state.token !== token ||
        (state.status !== 'running' && state.status !== 'invalidated')) {
      throw new Error('Session reconciliation ownership changed before invalidation');
    }
    writeState({ ...state, status: 'invalidated' });
    return sessionRepository.markActiveSessionsInterrupted();
  }).immediate();
}

/**
 * Clear the shared gate after live claims are restored.
 * Only the current owner token may complete, including a successful retry.
 */
export function completeSessionReconciliation(
  token: SessionReconciliationToken
): boolean {
  return getDatabase().transaction(() => {
    const state = readState();
    if (!token || !state || state.status === 'ready' || state.token !== token) return false;
    writeState({
      status: 'ready',
      completedAt: new Date().toISOString(),
    });
    return true;
  }).immediate();
}

/**
 * Persist a failed reconciliation so peer recovery stays blocked even after
 * the owner process exits. Only the current owner token may write failure.
 */
export function failSessionReconciliation(
  token: SessionReconciliationToken,
  error?: string
): boolean {
  return getDatabase().transaction(() => {
    const state = readState();
    if (!token || !state || state.token !== token ||
        (state.status !== 'running' && state.status !== 'invalidated')) return false;
    writeState({
      ...state,
      status: 'failed',
      failedAt: new Date().toISOString(),
      error,
    });
    return true;
  }).immediate();
}

/**
 * True while another Keepline process is mid invalidate-and-full-scan, or after
 * a failed reconciliation that has not yet been superseded by a successful one.
 * Dead owners are safe to ignore only before live claims have been invalidated.
 */
export function isSessionReconciliationRunning(): boolean {
  return getDatabase().transaction(() => {
    const state = readState();
    if (!state || state.status === 'ready') return false;
    if (state.status === 'failed') return true;
    if (typeof state.pid === 'number' && state.pid !== process.pid && !ownerIsLive(state)) {
      if (state.status === 'running') return false;
      writeState({
        ...state,
        status: 'failed',
        failedAt: new Date().toISOString(),
        error: 'Reconciliation owner exited after live claims were invalidated',
      });
    }
    return true;
  }).immediate();
}
