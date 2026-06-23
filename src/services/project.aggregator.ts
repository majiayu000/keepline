import type { AgentClient, SessionStatus } from '../domain/session/index.js';
import type { SessionUsageStats } from '../domain/session/value-objects.js';
import { getSessionStats } from './session.aggregator.js';
import type { AggregatedSession, BasicAggregatedSession } from './session.types.js';
import {
  runtimeIdForClient,
  type SessionRuntimeId,
} from './runtime-status.js';
import {
  resolveProjectIdentity,
  UNKNOWN_PROJECT_ID,
  UNKNOWN_PROJECT_ROOT,
  type ProjectIdentity,
} from './project.identity.js';

export type ProjectClient = AgentClient | 'unknown';
export type ProjectRuntime = SessionRuntimeId | 'unknown';
export type ProjectSession = AggregatedSession | BasicAggregatedSession;

export interface ProjectStatusStats {
  total: number;
  running: number;
  waiting: number;
  idle: number;
  lost: number;
  completed: number;
  withProcess: number;
}

export interface ProjectOverviewStats {
  total: number;
  active: number;
  idle: number;
}

export interface ProjectSummary {
  id: string;
  rootPath: string;
  path: string;
  name: string;
  displayPath: string;
  source: ProjectIdentity['source'];
  sessions: ProjectSession[];
  stats: ProjectStatusStats;
  clientCounts: Record<ProjectClient, number>;
  runtimeCounts: Record<ProjectRuntime, number>;
  currentTask?: string;
  lastActiveAt: Date;
  totalUsage?: SessionUsageStats;
}

export function aggregateProjectSummaries<T extends ProjectSession>(sessions: T[]): ProjectSummary[] {
  const projectMap = new Map<string, { identity: ProjectIdentity; sessions: T[] }>();

  for (const session of sessions) {
    const identity = resolveProjectIdentity(session.directory);
    const existing = projectMap.get(identity.id);
    if (existing) {
      existing.sessions.push(session);
    } else {
      projectMap.set(identity.id, { identity, sessions: [session] });
    }
  }

  return Array.from(projectMap.values())
    .map(({ identity, sessions: projectSessions }) => buildProjectSummary(identity, projectSessions))
    .sort((a, b) => b.lastActiveAt.getTime() - a.lastActiveAt.getTime());
}

export function getProjectOverviewStats(projects: ProjectSummary[]): ProjectOverviewStats {
  let active = 0;
  let idle = 0;

  for (const project of projects) {
    if (project.stats.running > 0 || project.stats.waiting > 0) {
      active++;
    } else {
      idle++;
    }
  }

  return {
    total: projects.length,
    active,
    idle,
  };
}

export function matchesProjectFilter(
  session: Pick<ProjectSession, 'directory'>,
  filter: { projectRoot?: string | null; projectId?: string | null }
): boolean {
  if (!filter.projectRoot && !filter.projectId) return true;

  const identity = resolveProjectIdentity(session.directory);
  if (filter.projectId && identity.id !== filter.projectId) {
    return false;
  }
  if (filter.projectRoot) {
    if (filter.projectRoot === UNKNOWN_PROJECT_ROOT) {
      return identity.id === UNKNOWN_PROJECT_ID;
    }
    const target = resolveProjectIdentity(filter.projectRoot);
    return identity.id === target.id;
  }
  return true;
}

function buildProjectSummary<T extends ProjectSession>(
  identity: ProjectIdentity,
  sessions: T[]
): ProjectSummary {
  return {
    id: identity.id,
    rootPath: identity.rootPath,
    path: identity.rootPath,
    name: identity.name,
    displayPath: identity.displayPath,
    source: identity.source,
    sessions,
    stats: getSessionStats(sessions) as ProjectStatusStats,
    clientCounts: countClients(sessions),
    runtimeCounts: countRuntimes(sessions),
    currentTask: findCurrentTask(sessions),
    lastActiveAt: findLastActive(sessions),
    totalUsage: aggregateUsageStats(sessions),
  };
}

function countClients(sessions: ProjectSession[]): Record<ProjectClient, number> {
  const counts: Record<ProjectClient, number> = {
    claude: 0,
    codex: 0,
    unknown: 0,
  };

  for (const session of sessions) {
    if (session.client === 'claude' || session.client === 'codex') {
      counts[session.client]++;
    } else {
      counts.unknown++;
    }
  }

  return counts;
}

function countRuntimes(sessions: ProjectSession[]): Record<ProjectRuntime, number> {
  const counts: Record<ProjectRuntime, number> = {
    'claude-code': 0,
    codex: 0,
    unknown: 0,
  };

  for (const session of sessions) {
    if (session.client === 'claude' || session.client === 'codex') {
      counts[runtimeIdForClient(session.client)]++;
    } else {
      counts.unknown++;
    }
  }

  return counts;
}

function findCurrentTask(sessions: ProjectSession[]): string | undefined {
  const active = sessions.filter((session) => isActiveTaskStatus(session.status));
  const candidates = active.length > 0 ? active : sessions;
  return sortByLastActive(candidates)[0]?.title || undefined;
}

function findLastActive(sessions: ProjectSession[]): Date {
  return sortByLastActive(sessions)[0]?.lastActiveAt ?? new Date(0);
}

function aggregateUsageStats(sessions: ProjectSession[]): SessionUsageStats | undefined {
  const withUsage = sessions.filter(
    (session): session is ProjectSession & { usageStats: SessionUsageStats } =>
      'usageStats' in session && session.usageStats !== undefined
  );

  if (withUsage.length === 0) return undefined;

  return withUsage.reduce<SessionUsageStats>(
    (acc, session) => ({
      totalInputTokens: acc.totalInputTokens + session.usageStats.totalInputTokens,
      totalOutputTokens: acc.totalOutputTokens + session.usageStats.totalOutputTokens,
      totalTokens: acc.totalTokens + session.usageStats.totalTokens,
      totalCost: acc.totalCost + session.usageStats.totalCost,
      apiCalls: acc.apiCalls + session.usageStats.apiCalls,
    }),
    {
      totalInputTokens: 0,
      totalOutputTokens: 0,
      totalTokens: 0,
      totalCost: 0,
      apiCalls: 0,
    }
  );
}

function sortByLastActive<T extends ProjectSession>(sessions: T[]): T[] {
  return [...sessions].sort((a, b) => b.lastActiveAt.getTime() - a.lastActiveAt.getTime());
}

function isActiveTaskStatus(status: SessionStatus): boolean {
  return status === 'running' || status === 'waiting';
}
