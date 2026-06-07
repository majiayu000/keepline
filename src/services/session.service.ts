/**
 * Session service - business logic for session management
 *
 * Optimized to use cached process data during sync cycles.
 * Includes sync lock to prevent concurrent sync operations.
 */

import type { Session } from '../domain/session/index.js';
import { generateTitle } from '../domain/session/index.js';
import type { ISessionRepository } from '../domain/session/repository.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';
import { emit } from '../lib/events.js';
import { getCachedProcesses, clearProcessCache } from '../adapters/process/scanner.js';
import { detectSessionStatus } from '../adapters/process/detector.js';
import { getAllSessions as getClaudeSessions } from '../adapters/claude/scanner.js';
import { logger } from '../lib/logger.js';
import { isValidSessionId } from '../lib/session-id.js';
import type { CreateSessionInput, UpdateSessionInput, AggregatedSession } from './session.types.js';
import { matchProcessesToSessions } from './session.process-matcher.js';

/** Options for sync operation */
export interface SyncOptions {
  maxAgeDays?: number; // Only sync files modified within this many days (default: 7 for fast sync)
  fullSync?: boolean; // Force full sync of all files
}

export class SessionService {
  private isSyncing = false;

  constructor(private readonly repository: ISessionRepository) {}

  /** Create a new session */
  createSession(input: CreateSessionInput): Session {
    const session = this.repository.upsert({
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
  updateSession(sessionId: string, input: UpdateSessionInput): Session {
    const existing = this.repository.findBySessionId(sessionId);
    const previousStatus = existing?.status;

    const session = this.repository.upsert({
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
  getAllSessions(): Session[] {
    return this.repository.findAll();
  }

  /** Get session by ID */
  getSession(sessionId: string): Session | null {
    return this.repository.findBySessionId(sessionId);
  }

  /** Get active sessions (not completed) */
  getActiveSessions(): Session[] {
    return this.repository.findActive();
  }

  /** Get sessions needing attention (lost or waiting) */
  getAttentionSessions(): Session[] {
    const lost = this.repository.findByStatus('lost');
    const waiting = this.repository.findByStatus('waiting');
    return [...lost, ...waiting];
  }

  /** Sync sessions with Claude data and running processes */
  async syncSessions(options: SyncOptions = {}): Promise<{
    discovered: number;
    updated: number;
    lost: number;
  }> {
    // Prevent concurrent sync operations
    if (this.isSyncing) {
      logger.debug('Sync already in progress, skipping');
      return { discovered: 0, updated: 0, lost: 0 };
    }

    this.isSyncing = true;
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
      const runningClaudePids = new Set(processes.map((process) => process.pid));

      // Get Claude sessions from file system (with optional age filter for performance)
      const scannedClaudeSessions = await getClaudeSessions({ maxAgeDays });
      const invalidClaudeSessions = scannedClaudeSessions.filter(
        (session) => !isValidSessionId(session.sessionId)
      );
      if (invalidClaudeSessions.length > 0) {
        logger.warn('Skipped scanned sessions with invalid session IDs before persistence', {
          count: invalidClaudeSessions.length,
          sample: invalidClaudeSessions.slice(0, 5).map((session) => ({
            sessionId: session.sessionId,
            directory: session.directory,
          })),
        });
      }
      const claudeSessions = scannedClaudeSessions.filter((session) =>
        isValidSessionId(session.sessionId)
      );
      const processMatches = matchProcessesToSessions(claudeSessions, processes);
      const existingSessions = this.repository.findBySessionIdsSummary(
        claudeSessions.map((session) => session.sessionId)
      );
      const existingSessionMap = new Map(
        existingSessions.map((session) => [session.sessionId, session])
      );

      // Process each Claude session
      for (const claudeSession of claudeSessions) {
        const existing = existingSessionMap.get(claudeSession.sessionId);
        const process = processMatches.get(claudeSession.sessionId);

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

          const updatedSession = this.repository.upsert({
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
            agentId: claudeSession.agentId,
            parentSessionId: claudeSession.parentSessionId,
            isSubAgent: claudeSession.isSubAgent,
            usageStats: claudeSession.usageStats,
            toolCalls: claudeSession.toolCalls,
          });
          existingSessionMap.set(claudeSession.sessionId, updatedSession);

          updated++;
          if (wasLost) {
            lost++;
            emit('session:lost', { session: updatedSession, previousStatus: existing.status });
          }
        } else {
          // Create new session with all data in single upsert (no redundant calls)
          const title = claudeSession.firstMessage
            ? generateTitle(claudeSession.firstMessage)
            : 'Unknown task';

          const newSession = this.repository.upsert({
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
            agentId: claudeSession.agentId,
            parentSessionId: claudeSession.parentSessionId,
            isSubAgent: claudeSession.isSubAgent,
            usageStats: claudeSession.usageStats,
            toolCalls: claudeSession.toolCalls,
          });
          existingSessionMap.set(claudeSession.sessionId, newSession);
          emit('session:discovered', { session: newSession });
          discovered++;
        }
      }

      // Check for sessions whose processes have died
      const activeSessions = this.repository.findActiveLightweight();
      for (const session of activeSessions) {
        if (session.pid && !runningClaudePids.has(session.pid)) {
          // Process died, mark as lost unless it was completed
          if (session.status !== 'completed') {
            const lostSession = this.repository.upsert({
              sessionId: session.sessionId,
              status: 'lost',
              pid: undefined,
            });
            lost++;
            emit('session:lost', {
              session: lostSession,
              previousStatus: session.status,
            });
          }
        }
      }

      const duration = Date.now() - startTime;
      if (discovered > 0 || lost > 0) {
        logger.info(`Session sync: ${discovered} new, ${lost} lost`);
      }
      emit('scan:complete', { sessionCount: claudeSessions.length, duration });
      logger.debug(`Sync complete: ${discovered} new, ${updated} updated, ${lost} lost (${duration}ms)`);

      return { discovered, updated, lost };
    } finally {
      this.isSyncing = false;
    }
  }

  /** Get aggregated session with process info (uses cached processes) */
  getAggregatedSession(sessionId: string): AggregatedSession | null {
    const session = this.repository.findBySessionId(sessionId);
    if (!session) return null;

    const sessions = this.repository.findAll();
    const processes = getCachedProcesses();
    const processMatches = matchProcessesToSessions(sessions, processes);
    const process = processMatches.get(session.sessionId);

    return {
      ...session,
      processRunning: !!process,
      cpuUsage: process?.cpu,
      memoryUsage: process?.memory,
    };
  }

  /** Mark session as completed */
  completeSession(sessionId: string): void {
    const session = this.repository.findBySessionId(sessionId);
    if (!session) return;

    this.repository.upsert({
      sessionId,
      status: 'completed',
      completedAt: new Date(),
      pid: undefined,
    });

    const completedSession = this.repository.findBySessionId(sessionId);
    if (completedSession) {
      emit('session:completed', { session: completedSession });
    }
  }
}

export const sessionService = new SessionService(sessionRepository);

export const createSession = sessionService.createSession.bind(sessionService);
export const updateSession = sessionService.updateSession.bind(sessionService);
export const getAllSessions = sessionService.getAllSessions.bind(sessionService);
export const getSession = sessionService.getSession.bind(sessionService);
export const getActiveSessions = sessionService.getActiveSessions.bind(sessionService);
export const getAttentionSessions = sessionService.getAttentionSessions.bind(sessionService);
export const syncSessions = sessionService.syncSessions.bind(sessionService);
export const getAggregatedSession = sessionService.getAggregatedSession.bind(sessionService);
export const completeSession = sessionService.completeSession.bind(sessionService);
