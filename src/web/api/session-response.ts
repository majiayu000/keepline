import type { AggregatedSession } from '../../services/session.types.js';

export function serializeBasicSession(session: AggregatedSession) {
  return {
    id: session.id,
    sessionId: session.sessionId,
    directory: session.directory,
    status: session.status,
    title: session.title,
    lastActiveAt: session.lastActiveAt.toISOString(),
    startedAt: session.startedAt?.toISOString(),
    completedAt: session.completedAt?.toISOString(),
    createdAt: session.createdAt.toISOString(),
    updatedAt: session.updatedAt.toISOString(),
    pid: session.pid,
    tty: session.tty,
    toolCount: session.toolCount,
    messageCount: session.messageCount,
    processRunning: session.processRunning,
    cpuUsage: session.cpuUsage,
    memoryUsage: session.memoryUsage,
  };
}

export function serializeBasicSessions(sessions: AggregatedSession[]) {
  return sessions.map(serializeBasicSession);
}
