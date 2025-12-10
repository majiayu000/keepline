/**
 * Session aggregator - combines data from multiple sources
 */

import type { SessionStatus } from '../core/types.js';
import { sessionRepo } from '../storage/index.js';
import { scanClaudeProcesses } from '../process/scanner.js';
import { detectSessionStatus } from '../process/detector.js';
import type { AggregatedSession, SessionFilter, SessionSort } from './types.js';
import type { ClaudeProcessInfo } from '../process/types.js';
import { logger } from '../utils/logger.js';

// Process cache with TTL
const CACHE_TTL_MS = 30000; // 30 seconds
let processCache: ClaudeProcessInfo[] | null = null;
let processCacheTimestamp = 0;

/** Get cached Claude processes */
function getCachedProcesses(): ClaudeProcessInfo[] {
  const now = Date.now();
  if (processCache && now - processCacheTimestamp < CACHE_TTL_MS) {
    return processCache;
  }

  logger.debug('Process cache miss, rescanning...');
  processCache = scanClaudeProcesses();
  processCacheTimestamp = now;
  return processCache;
}

/** Invalidate process cache (call when process state changes) */
export function invalidateProcessCache(): void {
  processCache = null;
  processCacheTimestamp = 0;
}

/** Get all sessions with aggregated process info */
export function getAggregatedSessions(): AggregatedSession[] {
  const sessions = sessionRepo.findAll();
  const processes = getCachedProcesses();
  const processByCwd = new Map(processes.map((p) => [p.cwd, p]));

  return sessions.map((session) => {
    const process = processByCwd.get(session.directory);
    const liveStatus = detectSessionStatus(process || null, session.lastActiveAt);

    return {
      ...session,
      // Update status based on live process info
      status: session.status === 'completed' ? 'completed' : liveStatus,
      processRunning: !!process,
      cpuUsage: process?.cpu,
      memoryUsage: process?.memory,
    };
  });
}

/** Filter sessions */
export function filterSessions(
  sessions: AggregatedSession[],
  filter: SessionFilter
): AggregatedSession[] {
  let result = sessions;

  if (filter.status && filter.status.length > 0) {
    result = result.filter((s) => filter.status!.includes(s.status));
  }

  if (filter.directory) {
    result = result.filter((s) => s.directory.includes(filter.directory!));
  }

  if (filter.hasProcess !== undefined) {
    result = result.filter((s) => s.processRunning === filter.hasProcess);
  }

  if (filter.limit && filter.limit > 0) {
    result = result.slice(0, filter.limit);
  }

  return result;
}

/** Sort sessions */
export function sortSessions(
  sessions: AggregatedSession[],
  sort: SessionSort
): AggregatedSession[] {
  const sorted = [...sessions];

  sorted.sort((a, b) => {
    let comparison = 0;

    switch (sort.field) {
      case 'lastActiveAt':
        comparison = a.lastActiveAt.getTime() - b.lastActiveAt.getTime();
        break;
      case 'startedAt':
        const aStart = a.startedAt?.getTime() || 0;
        const bStart = b.startedAt?.getTime() || 0;
        comparison = aStart - bStart;
        break;
      case 'directory':
        comparison = a.directory.localeCompare(b.directory);
        break;
      case 'status':
        comparison = a.status.localeCompare(b.status);
        break;
    }

    return sort.order === 'desc' ? -comparison : comparison;
  });

  return sorted;
}

/** Group sessions by status */
export function groupByStatus(
  sessions: AggregatedSession[]
): Map<SessionStatus, AggregatedSession[]> {
  const grouped = new Map<SessionStatus, AggregatedSession[]>();

  for (const session of sessions) {
    const existing = grouped.get(session.status) || [];
    existing.push(session);
    grouped.set(session.status, existing);
  }

  return grouped;
}

/** Group sessions by directory */
export function groupByDirectory(
  sessions: AggregatedSession[]
): Map<string, AggregatedSession[]> {
  const grouped = new Map<string, AggregatedSession[]>();

  for (const session of sessions) {
    const existing = grouped.get(session.directory) || [];
    existing.push(session);
    grouped.set(session.directory, existing);
  }

  return grouped;
}

/** Get session statistics - single pass for efficiency */
export function getSessionStats(sessions: AggregatedSession[]): {
  total: number;
  running: number;
  waiting: number;
  idle: number;
  lost: number;
  completed: number;
  withProcess: number;
} {
  const stats = {
    total: sessions.length,
    running: 0,
    waiting: 0,
    idle: 0,
    lost: 0,
    completed: 0,
    withProcess: 0,
  };

  // Single pass through sessions
  for (const session of sessions) {
    switch (session.status) {
      case 'running':
        stats.running++;
        break;
      case 'waiting':
        stats.waiting++;
        break;
      case 'idle':
        stats.idle++;
        break;
      case 'lost':
        stats.lost++;
        break;
      case 'completed':
        stats.completed++;
        break;
    }
    if (session.processRunning) {
      stats.withProcess++;
    }
  }

  return stats;
}
