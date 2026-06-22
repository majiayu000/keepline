import type { ProjectSession, ProjectSummary } from '../../services/project.aggregator.js';
import type { AggregatedSession } from '../../services/session.types.js';
import { serializeBasicSessions, serializeFullSessions } from './session-response.js';

interface SerializeProjectSummaryOptions {
  includeSessions?: boolean;
  sessionFields?: 'basic' | 'full';
}

export function serializeProjectSummary(
  project: ProjectSummary,
  options: SerializeProjectSummaryOptions = {}
) {
  return {
    id: project.id,
    rootPath: project.rootPath,
    path: project.path,
    name: project.name,
    displayPath: project.displayPath,
    source: project.source,
    ...(options.includeSessions ? { sessions: serializeProjectSessions(project.sessions, options) } : {}),
    stats: project.stats,
    clientCounts: project.clientCounts,
    currentTask: project.currentTask,
    lastActiveAt: project.lastActiveAt.toISOString(),
    totalUsage: project.totalUsage,
  };
}

function serializeProjectSessions(
  sessions: ProjectSession[],
  options: SerializeProjectSummaryOptions
) {
  if (options.sessionFields === 'full') {
    return serializeFullSessions(sessions as AggregatedSession[]);
  }
  return serializeBasicSessions(sessions);
}

export function serializeProjectSummaries(
  projects: ProjectSummary[],
  options: SerializeProjectSummaryOptions = {}
) {
  return projects.map(project => serializeProjectSummary(project, options));
}
