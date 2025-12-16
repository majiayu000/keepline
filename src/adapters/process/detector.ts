/**
 * Session state detector based on process activity
 *
 * Thresholds are configurable via config file
 */

import type { SessionStatus } from '../../lib/types.js';
import type { ClaudeProcessInfo } from './types.js';
import { config } from '../../lib/config.js';

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
    waiting: 'Waiting for user input',
    idle: 'Idle but running',
    lost: 'Process terminated unexpectedly',
    completed: 'Session completed',
  };
  return descriptions[status];
}

/** Determine if session needs attention */
export function needsAttention(status: SessionStatus): boolean {
  return status === 'lost' || status === 'waiting';
}
