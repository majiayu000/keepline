/**
 * Project types - for grouping sessions by directory
 */

import type { AgentClient, Session, SessionRuntimeId, SessionStatus, UsageStats } from './session'

export type ProjectClient = AgentClient | 'unknown'
export type ProjectRuntime = SessionRuntimeId | 'unknown'

/** Statistics for a single project */
export interface ProjectStats {
  running: number
  waiting: number
  idle: number
  lost: number
  completed: number
  total: number
}

/** Aggregated project information */
export interface ProjectInfo {
  /** Stable project identity */
  id: string
  /** Canonical project root path */
  rootPath: string
  /** Full directory path */
  path: string
  /** Compact display path */
  displayPath: string
  /** Extracted project name (last segment of path) */
  name: string
  /** Project identity source */
  source?: 'git-root' | 'cwd' | 'unknown'
  /** Sessions are only present in full project responses */
  sessions?: Session[]
  /** Session status counts */
  stats: ProjectStats
  /** Session counts by agent client */
  clientCounts: Record<ProjectClient, number>
  /** Session counts by runtime adapter */
  runtimeCounts: Record<ProjectRuntime, number>
  /** Title/prompt of the most recently active session */
  currentTask?: string
  /** Most recent activity timestamp */
  lastActiveAt: string
  /** Aggregated usage stats across all sessions */
  totalUsage?: UsageStats
}

/** Overall project statistics */
export interface ProjectOverviewStats {
  /** Total number of projects */
  total: number
  /** Projects with at least one running/waiting session */
  active: number
  /** Projects with only idle/lost/completed sessions */
  idle: number
}

/** Helper type for status-based styling */
export type ProjectActivityStatus = 'active' | 'idle' | 'inactive'

/**
 * Determine the activity status of a project based on its session stats
 */
export function getProjectActivityStatus(stats: ProjectStats): ProjectActivityStatus {
  if (stats.running > 0 || stats.waiting > 0) {
    return 'active'
  }
  if (stats.idle > 0) {
    return 'idle'
  }
  return 'inactive'
}

/**
 * Extract project name from a full path
 * @example "/Users/xxx/code/keepline" → "keepline"
 */
export function extractProjectName(path: string): string {
  const segments = path.split('/').filter(Boolean)
  return segments[segments.length - 1] || path
}

export function normalizeProjectPath(path: string): string {
  const trimmed = path.trim()
  if (!trimmed) return 'Unknown'
  const withoutTrailing = trimmed.replace(/\/+$/, '')
  return withoutTrailing || trimmed
}

export function projectIdFromPath(path: string): string {
  return `path:${encodeURIComponent(normalizeProjectPath(path))}`
}

export function createClientCounts(sessions: Session[]): Record<ProjectClient, number> {
  const counts: Record<ProjectClient, number> = {
    claude: 0,
    codex: 0,
    unknown: 0,
  }

  for (const session of sessions) {
    if (session.client === 'claude' || session.client === 'codex') {
      counts[session.client]++
    } else {
      counts.unknown++
    }
  }

  return counts
}

export function createRuntimeCounts(sessions: Session[]): Record<ProjectRuntime, number> {
  const counts: Record<ProjectRuntime, number> = {
    'claude-code': 0,
    codex: 0,
    unknown: 0,
  }

  for (const session of sessions) {
    if (session.runtimeId === 'claude-code' || session.runtimeId === 'codex') {
      counts[session.runtimeId]++
    } else {
      counts.unknown++
    }
  }

  return counts
}

/**
 * Aggregate sessions by exact directory path for tests and full-list fallbacks.
 */
export function aggregateProjects(sessions: Session[]): ProjectInfo[] {
  const projectMap = new Map<string, Session[]>()

  for (const session of sessions) {
    const directory = normalizeProjectPath(session.directory || 'Unknown')
    const existing = projectMap.get(directory) || []
    projectMap.set(directory, [...existing, session])
  }

  const projects: ProjectInfo[] = Array.from(projectMap.entries()).map(
    ([path, projectSessions]) => {
      const sortedByTime = [...projectSessions].sort(
        (a, b) => new Date(b.lastActiveAt).getTime() - new Date(a.lastActiveAt).getTime()
      )
      const primaryPath = normalizeProjectPath(sortedByTime[0]?.directory || path)

      return {
        id: projectIdFromPath(path),
        rootPath: path,
        path: primaryPath,
        displayPath: path,
        name: extractProjectName(path),
        source: path === 'Unknown' ? 'unknown' : 'cwd',
        sessions: projectSessions,
        stats: calculateProjectStats(projectSessions),
        clientCounts: createClientCounts(projectSessions),
        runtimeCounts: createRuntimeCounts(projectSessions),
        currentTask: findCurrentTask(projectSessions),
        lastActiveAt: findLastActive(projectSessions),
        totalUsage: aggregateUsageStats(projectSessions),
      }
    }
  )

  return projects.sort(
    (a, b) => new Date(b.lastActiveAt).getTime() - new Date(a.lastActiveAt).getTime()
  )
}

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

/**
 * Calculate project stats from sessions
 */
export function calculateProjectStats(sessions: Session[]): ProjectStats {
  const stats: ProjectStats = {
    running: 0,
    waiting: 0,
    idle: 0,
    lost: 0,
    completed: 0,
    total: sessions.length,
  }

  for (const session of sessions) {
    const status = session.status as SessionStatus
    if (status in stats) {
      stats[status]++
    }
  }

  return stats
}

/**
 * Find the current task (most recent active session's title/prompt)
 */
export function findCurrentTask(sessions: Session[]): string | undefined {
  // Find the most recently active session that's running or waiting
  const activeSessions = sessions.filter(
    (s) => s.status === 'running' || s.status === 'waiting'
  )

  if (activeSessions.length === 0) {
    // Fall back to most recent session
    const sorted = [...sessions].sort(
      (a, b) => new Date(b.lastActiveAt).getTime() - new Date(a.lastActiveAt).getTime()
    )
    return sorted[0]?.title || sorted[0]?.initialPrompt
  }

  // Sort by last active time
  const sorted = activeSessions.sort(
    (a, b) => new Date(b.lastActiveAt).getTime() - new Date(a.lastActiveAt).getTime()
  )

  return sorted[0]?.title || sorted[0]?.initialPrompt
}

/**
 * Find the most recent activity timestamp
 */
export function findLastActive(sessions: Session[]): string {
  if (sessions.length === 0) {
    return new Date().toISOString()
  }

  const sorted = [...sessions].sort(
    (a, b) => new Date(b.lastActiveAt).getTime() - new Date(a.lastActiveAt).getTime()
  )

  return sorted[0].lastActiveAt
}

/**
 * Aggregate usage stats from all sessions
 */
export function aggregateUsageStats(sessions: Session[]): UsageStats | undefined {
  const sessionsWithStats = sessions.filter((s) => s.usageStats)

  if (sessionsWithStats.length === 0) {
    return undefined
  }

  return sessionsWithStats.reduce(
    (acc, session) => {
      const stats = session.usageStats!
      return {
        totalInputTokens: acc.totalInputTokens + (stats.totalInputTokens || 0),
        totalOutputTokens: acc.totalOutputTokens + (stats.totalOutputTokens || 0),
        totalTokens: acc.totalTokens + (stats.totalTokens || 0),
        totalCost: acc.totalCost + (stats.totalCost || 0),
        apiCalls: acc.apiCalls + (stats.apiCalls || 0),
      }
    },
    {
      totalInputTokens: 0,
      totalOutputTokens: 0,
      totalTokens: 0,
      totalCost: 0,
      apiCalls: 0,
    }
  )
}
