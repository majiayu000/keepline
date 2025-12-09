/**
 * Session service - business logic for session management
 */

import type { Session } from '../core/types.js';
import { emit } from '../core/events.js';
import { sessionRepo } from '../storage/index.js';
import { scanClaudeProcesses, isProcessRunning } from '../process/scanner.js';
import { detectSessionStatus } from '../process/detector.js';
import { getAllSessions as getClaudeSessions } from '../claude/scanner.js';
import { logger } from '../utils/logger.js';
import type { CreateSessionInput, UpdateSessionInput, AggregatedSession } from './types.js';

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

/** Sync sessions with Claude data and running processes */
export async function syncSessions(): Promise<{
  discovered: number;
  updated: number;
  lost: number;
}> {
  const startTime = Date.now();
  let discovered = 0;
  let updated = 0;
  let lost = 0;

  // Get current process info
  const processes = scanClaudeProcesses();
  const processByCwd = new Map(processes.map((p) => [p.cwd, p]));

  // Get all Claude sessions from file system
  const claudeSessions = await getClaudeSessions();

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

      sessionRepo.upsert({
        sessionId: claudeSession.sessionId,
        status,
        lastTool: claudeSession.lastTool,
        lastToolInput: claudeSession.lastToolInput
          ? JSON.stringify(claudeSession.lastToolInput)
          : undefined,
        currentFile: claudeSession.currentFile,
        lastActiveAt: claudeSession.lastActiveAt,
        pid: process?.pid,
        tty: process?.tty,
        toolCount: claudeSession.toolCount,
        messageCount: claudeSession.messageCount,
      });

      updated++;
      if (wasLost) {
        lost++;
        const session = sessionRepo.findBySessionId(claudeSession.sessionId)!;
        emit('session:lost', { session, previousStatus: existing.status });
      }
    } else {
      // Create new session
      createSession({
        sessionId: claudeSession.sessionId,
        directory: claudeSession.directory,
        initialPrompt: claudeSession.firstMessage || 'Unknown task',
        pid: process?.pid,
        tty: process?.tty,
      });

      // Update with full data
      sessionRepo.upsert({
        sessionId: claudeSession.sessionId,
        status,
        lastTool: claudeSession.lastTool,
        lastToolInput: claudeSession.lastToolInput
          ? JSON.stringify(claudeSession.lastToolInput)
          : undefined,
        currentFile: claudeSession.currentFile,
        startedAt: claudeSession.startedAt,
        lastActiveAt: claudeSession.lastActiveAt,
        toolCount: claudeSession.toolCount,
        messageCount: claudeSession.messageCount,
      });

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
        emit('session:lost', { session: sessionRepo.findBySessionId(session.sessionId)!, previousStatus: session.status });
      }
    }
  }

  const duration = Date.now() - startTime;
  emit('scan:complete', { sessionCount: claudeSessions.length, duration });
  logger.debug(`Sync complete: ${discovered} new, ${updated} updated, ${lost} lost (${duration}ms)`);

  return { discovered, updated, lost };
}

/** Get aggregated session with process info */
export function getAggregatedSession(sessionId: string): AggregatedSession | null {
  const session = sessionRepo.findBySessionId(sessionId);
  if (!session) return null;

  const processes = scanClaudeProcesses();
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

  emit('session:completed', { session: sessionRepo.findBySessionId(sessionId)! });
}
