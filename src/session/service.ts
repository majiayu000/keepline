/**
 * Session service - business logic for session management
 *
 * Optimized to use cached process data during sync cycles.
 * Includes sync lock to prevent concurrent sync operations.
 */

import type { Session } from '../core/types.js';
import { emit } from '../core/events.js';
import { sessionRepo } from '../storage/index.js';
import { getCachedProcesses, clearProcessCache, isProcessRunning } from '../process/scanner.js';
import { detectSessionStatus } from '../process/detector.js';
import { getAllSessions as getClaudeSessions } from '../claude/scanner.js';
import { logger } from '../utils/logger.js';
import type { CreateSessionInput, UpdateSessionInput, AggregatedSession } from './types.js';

// Sync lock to prevent concurrent sync operations
let isSyncing = false;

/** Generate title from prompt */
function generateTitle(prompt: string): string {
  // Take first 80 chars, trim to last complete word
  if (prompt.length <= 80) return prompt;

  const truncated = prompt.slice(0, 80);
  const lastSpace = truncated.lastIndexOf(' ');
  if (lastSpace > 40) {
    return truncated.slice(0, lastSpace) + '...';
  }
  return truncated + '...';
}

/** Create a new session */
export function createSession(input: CreateSessionInput): Session {
  const session = sessionRepo.upsert({
    sessionId: input.sessionId,
    directory: input.directory,
    initialPrompt: input.initialPrompt,
    title: input.title || generateTitle(input.initialPrompt),
    status: 'running',
    pid: input.pid,
    tty: input.tty,
    lastActiveAt: new Date(),
    startedAt: new Date(),
    toolCount: 0,
    messageCount: 1,
  });

  emit('session:discovered', { session });
  logger.info(`Session created: ${session.sessionId}`);
  return session;
}

/** Update an existing session */
export function updateSession(sessionId: string, input: UpdateSessionInput): Session {
  const existing = sessionRepo.findBySessionId(sessionId);
  const previousStatus = existing?.status;

  const session = sessionRepo.upsert({
    sessionId,
    ...input,
  });

  if (previousStatus && previousStatus !== session.status) {
    emit('session:updated', { session, previousStatus });
    logger.debug(`Session ${sessionId} status: ${previousStatus} -> ${session.status}`);
  }

  return session;
}

/** Get all sessions */
export function getAllSessions(): Session[] {
  return sessionRepo.findAll();
}

/** Get session by ID */
export function getSession(sessionId: string): Session | null {
  return sessionRepo.findBySessionId(sessionId);
}

/** Get active sessions (not completed) */
export function getActiveSessions(): Session[] {
  return sessionRepo.findActive();
}

/** Get sessions needing attention (lost or waiting) */
export function getAttentionSessions(): Session[] {
  const lost = sessionRepo.findByStatus('lost');
  const waiting = sessionRepo.findByStatus('waiting');
  return [...lost, ...waiting];
}

/** Options for sync operation */
export interface SyncOptions {
  maxAgeDays?: number; // Only sync files modified within this many days (default: 7 for fast sync)
  fullSync?: boolean; // Force full sync of all files
}

/** Sync sessions with Claude data and running processes */
export async function syncSessions(options: SyncOptions = {}): Promise<{
  discovered: number;
  updated: number;
  lost: number;
}> {
  // Prevent concurrent sync operations
  if (isSyncing) {
    logger.debug('Sync already in progress, skipping');
    return { discovered: 0, updated: 0, lost: 0 };
  }

  isSyncing = true;
  const startTime = Date.now();
  let discovered = 0;
  let updated = 0;
  let lost = 0;

  // Default to 7 days for fast sync, unless fullSync is requested
  const maxAgeDays = options.fullSync ? undefined : (options.maxAgeDays ?? 7);

  try {
    // Clear process cache at start of sync cycle to ensure fresh data
    clearProcessCache();

    // Get current process info (will be cached for duration of sync)
    const processes = getCachedProcesses();
    const processByCwd = new Map(processes.map((p) => [p.cwd, p]));

    // Get Claude sessions from file system (with optional age filter for performance)
    const claudeSessions = await getClaudeSessions({ maxAgeDays });

    // Process each Claude session
    for (const claudeSession of claudeSessions) {
      const existing = sessionRepo.findBySessionId(claudeSession.sessionId);
      const process = processByCwd.get(claudeSession.directory);

      const status = detectSessionStatus(
        process || null,
        claudeSession.lastActiveAt
      );

      if (existing) {
        // Update existing session
        const wasLost = existing.status !== 'lost' && status === 'lost';

        // Update title if current one is Unknown and we have new info
        const shouldUpdateTitle =
          existing.title === 'Unknown task' &&
          claudeSession.firstMessage &&
          claudeSession.firstMessage !== 'Unknown task';

        sessionRepo.upsert({
          sessionId: claudeSession.sessionId,
          status,
          ...(shouldUpdateTitle && {
            title: generateTitle(claudeSession.firstMessage!),
            initialPrompt: claudeSession.firstMessage,
          }),
          lastTool: claudeSession.lastTool,
          lastToolInput: claudeSession.lastToolInput
            ? JSON.stringify(claudeSession.lastToolInput)
            : undefined,
          currentFile: claudeSession.currentFile,
          lastMessage: claudeSession.lastMessage,
          lastActiveAt: claudeSession.lastActiveAt,
          pid: process?.pid,
          tty: process?.tty,
          toolCount: claudeSession.toolCount,
          messageCount: claudeSession.messageCount,
        });

        updated++;
        if (wasLost) {
          lost++;
          const updatedSession = sessionRepo.findBySessionId(claudeSession.sessionId);
          if (updatedSession) {
            emit('session:lost', { session: updatedSession, previousStatus: existing.status });
          }
        }
      } else {
        // Create new session with all data in single upsert (no redundant calls)
        const title = claudeSession.firstMessage
          ? generateTitle(claudeSession.firstMessage)
          : 'Unknown task';

        sessionRepo.upsert({
          sessionId: claudeSession.sessionId,
          directory: claudeSession.directory,
          initialPrompt: claudeSession.firstMessage || 'Unknown task',
          title,
          status,
          lastTool: claudeSession.lastTool,
          lastToolInput: claudeSession.lastToolInput
            ? JSON.stringify(claudeSession.lastToolInput)
            : undefined,
          currentFile: claudeSession.currentFile,
          lastMessage: claudeSession.lastMessage,
          startedAt: claudeSession.startedAt,
          lastActiveAt: claudeSession.lastActiveAt,
          pid: process?.pid,
          tty: process?.tty,
          toolCount: claudeSession.toolCount,
          messageCount: claudeSession.messageCount,
        });

        const newSession = sessionRepo.findBySessionId(claudeSession.sessionId);
        if (newSession) {
          emit('session:discovered', { session: newSession });
          logger.info(`Session discovered: ${claudeSession.sessionId}`);
        }
        discovered++;
      }
    }

    // Check for sessions whose processes have died
    const activeSessions = sessionRepo.findActive();
    for (const session of activeSessions) {
      if (session.pid && !isProcessRunning(session.pid)) {
        // Process died, mark as lost unless it was completed
        if (session.status !== 'completed') {
          sessionRepo.upsert({
            sessionId: session.sessionId,
            status: 'lost',
            pid: undefined,
          });
          lost++;
          const lostSession = sessionRepo.findBySessionId(session.sessionId);
          if (lostSession) {
            emit('session:lost', {
              session: lostSession,
              previousStatus: session.status,
            });
          }
        }
      }
    }

    const duration = Date.now() - startTime;
    emit('scan:complete', { sessionCount: claudeSessions.length, duration });
    logger.debug(`Sync complete: ${discovered} new, ${updated} updated, ${lost} lost (${duration}ms)`);

    return { discovered, updated, lost };
  } finally {
    isSyncing = false;
  }
}

/** Get aggregated session with process info (uses cached processes) */
export function getAggregatedSession(sessionId: string): AggregatedSession | null {
  const session = sessionRepo.findBySessionId(sessionId);
  if (!session) return null;

  const processes = getCachedProcesses();
  const process = processes.find((p) => p.cwd === session.directory);

  return {
    ...session,
    processRunning: !!process,
    cpuUsage: process?.cpu,
    memoryUsage: process?.memory,
  };
}

/** Mark session as completed */
export function completeSession(sessionId: string): void {
  const session = sessionRepo.findBySessionId(sessionId);
  if (!session) return;

  sessionRepo.upsert({
    sessionId,
    status: 'completed',
    completedAt: new Date(),
    pid: undefined,
  });

  const completedSession = sessionRepo.findBySessionId(sessionId);
  if (completedSession) {
    emit('session:completed', { session: completedSession });
  }
}
