import { useMemo } from 'react'
import type { Session } from '@/types'
import type { ProjectInfo, ProjectOverviewStats } from '@/types/project'
import {
  extractProjectName,
  calculateProjectStats,
  findCurrentTask,
  findLastActive,
  aggregateUsageStats,
} from '@/types/project'

export interface UseProjectsReturn {
  /** List of projects, sorted by last activity */
  projects: ProjectInfo[]
  /** Overview statistics */
  stats: ProjectOverviewStats
  /** Whether any projects exist */
  hasProjects: boolean
}

/**
 * Aggregates sessions by directory into project groups.
 *
 * @param sessions - Array of sessions to aggregate
 * @returns Aggregated project data with statistics
 */
export function useProjects(sessions: Session[]): UseProjectsReturn {
  const projects = useMemo(() => {
    return aggregateProjects(sessions)
  }, [sessions])

  const stats = useMemo(() => {
    return calculateOverviewStats(projects)
  }, [projects])

  return {
    projects,
    stats,
    hasProjects: projects.length > 0,
  }
}

/**
 * Aggregate sessions by project name (not full path).
 * This merges sessions from different directories with the same project name.
 * Exported for testing.
 */
export function aggregateProjects(sessions: Session[]): ProjectInfo[] {
  // Group sessions by project name (last segment of path)
  const projectMap = new Map<string, Session[]>()

  for (const session of sessions) {
    const directory = session.directory || 'Unknown'
    const projectName = extractProjectName(directory)
    const existing = projectMap.get(projectName) || []
    projectMap.set(projectName, [...existing, session])
  }

  // Transform to ProjectInfo array
  const projects: ProjectInfo[] = Array.from(projectMap.entries()).map(
    ([name, projectSessions]) => {
      // Use the most recent session's directory as the primary path
      const sortedByTime = [...projectSessions].sort(
        (a, b) => new Date(b.lastActiveAt).getTime() - new Date(a.lastActiveAt).getTime()
      )
      const primaryPath = sortedByTime[0]?.directory || 'Unknown'

      return {
        path: primaryPath,
        name,
        sessions: projectSessions,
        stats: calculateProjectStats(projectSessions),
        currentTask: findCurrentTask(projectSessions),
        lastActiveAt: findLastActive(projectSessions),
        totalUsage: aggregateUsageStats(projectSessions),
      }
    }
  )

  // Sort by last activity (most recent first)
  return projects.sort(
    (a, b) => new Date(b.lastActiveAt).getTime() - new Date(a.lastActiveAt).getTime()
  )
}

/**
 * Calculate overview statistics from projects.
 * Exported for testing.
 */
export function calculateOverviewStats(projects: ProjectInfo[]): ProjectOverviewStats {
  let active = 0
  let idle = 0

  for (const project of projects) {
    if (project.stats.running > 0 || project.stats.waiting > 0) {
      active++
    } else {
      idle++
    }
  }

  return {
    total: projects.length,
    active,
    idle,
  }
}
