import type { AggregatedSession, BasicAggregatedSession } from '../../services/session.types.js';
import { runtimeIdForClient } from '../../services/runtime-status.js';

type SerializableBasicSession = AggregatedSession | BasicAggregatedSession;

export function serializeBasicSession(session: SerializableBasicSession) {
  return {
    id: session.id,
    sessionId: session.sessionId,
    client: session.client,
    runtimeId: runtimeIdForClient(session.client),
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
    usageStats: session.usageStats,
    processRunning: session.processRunning,
    cpuUsage: session.cpuUsage,
    memoryUsage: session.memoryUsage,
  };
}

export function serializeBasicSessions(sessions: SerializableBasicSession[]) {
  return sessions.map(serializeBasicSession);
}

export function serializeFullSession(session: AggregatedSession) {
  return {
    ...session,
    runtimeId: runtimeIdForClient(session.client),
    lastActiveAt: session.lastActiveAt.toISOString(),
    startedAt: session.startedAt?.toISOString(),
    completedAt: session.completedAt?.toISOString(),
    createdAt: session.createdAt.toISOString(),
    updatedAt: session.updatedAt.toISOString(),
    usageStats: session.usageStats,
  };
}

export function serializeFullSessions(sessions: AggregatedSession[]) {
  return sessions.map(serializeFullSession);
}
