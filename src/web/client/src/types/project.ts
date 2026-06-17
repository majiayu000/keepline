/**
 * Project types - for grouping sessions by directory
 */

import type { Session, SessionStatus, UsageStats } from './session'

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
  /** Full directory path */
  path: string
  /** Extracted project name (last segment of path) */
  name: string
  /** All sessions in this directory */
  sessions: Session[]
  /** Session status counts */
  stats: ProjectStats
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
