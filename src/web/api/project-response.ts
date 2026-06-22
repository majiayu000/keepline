import type { ProjectSummary } from '../../services/project.aggregator.js';
import { serializeBasicSessions } from './session-response.js';

interface SerializeProjectSummaryOptions {
  includeSessions?: boolean;
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
    ...(options.includeSessions ? { sessions: serializeBasicSessions(project.sessions) } : {}),
    stats: project.stats,
    clientCounts: project.clientCounts,
    currentTask: project.currentTask,
    lastActiveAt: project.lastActiveAt.toISOString(),
    totalUsage: project.totalUsage,
  };
}

export function serializeProjectSummaries(
  projects: ProjectSummary[],
  options: SerializeProjectSummaryOptions = {}
) {
  return projects.map(project => serializeProjectSummary(project, options));
}
