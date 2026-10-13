/**
 * Session state detector based on process activity
 *
 * Thresholds are configurable via config file
 */

import type { Session, SessionStatus } from '../../domain/session/index.js';
import type { ClaudeProcessInfo } from './types.js';
import { config } from '../../lib/config.js';

/** A missing match is not proof of exit, especially for a shared desktop server. */
export function detectProcessState(
  matched: boolean,
  previous?: Pick<Session, 'pid' | 'processState'>,
  probe: (pid: number, signal: 0) => unknown = process.kill,
): NonNullable<Session['processState']> {
  if (matched) return 'online';
  if (!previous?.pid) return previous?.processState === 'exited' ? 'exited' : 'unknown';
  try { probe(previous.pid, 0); return 'unknown'; }
  catch (error) { return (error as NodeJS.ErrnoException).code === 'ESRCH' ? 'exited' : 'unknown'; }
}

/** Detect session status based on process info */
export function detectSessionStatus(
  process: ClaudeProcessInfo | null,
  lastActivityAt?: Date
): SessionStatus {
  const cfg = config.get();

  // No process = session is lost
  if (!process) {
    return 'lost';
  }

  // High CPU = actively running
  if (process.cpu > cfg.activeCpuThreshold) {
    return 'running';
  }

  // Check time since last activity
  if (lastActivityAt) {
    const secondsSinceActivity = (Date.now() - lastActivityAt.getTime()) / 1000;

    // Very recent activity = running
    if (secondsSinceActivity < cfg.runningThresholdSeconds) {
      return 'running';
    }

    // Some activity = waiting for input
    if (secondsSinceActivity < cfg.idleThresholdSeconds) {
      return 'waiting';
    }
  }

  // Low CPU, no recent activity = idle
  return 'idle';
}

/** Check if session appears to be completed */
export function isSessionCompleted(
  process: ClaudeProcessInfo | null,
  lastTool?: string
): boolean {
  // If process is still running, not completed
  if (process) return false;

  // If no process and last tool suggests completion
  const completionTools = ['TodoWrite', 'Write', 'Edit'];
  if (lastTool && completionTools.includes(lastTool)) {
    return true;
  }

  return false;
}

/** Get status description */
export function getStatusDescription(status: SessionStatus): string {
  const descriptions: Record<SessionStatus, string> = {
    running: 'Actively processing',
    needs_input: 'Waiting for user input',
    waiting: 'Waiting for user input',
    idle: 'Idle but running',
    stalled: 'No recent turn activity',
    interrupted: 'Turn interrupted',
    lost: 'Process terminated unexpectedly',
    completed: 'Session completed',
  };
  return descriptions[status];
}

/** Determine if session needs attention */
export function needsAttention(status: SessionStatus): boolean {
  return status === 'lost' || status === 'waiting';
}
