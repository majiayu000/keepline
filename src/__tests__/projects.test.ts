/**
 * Integration tests for Projects View functionality
 *
 * These tests verify project aggregation logic using real data structures.
 * Following best practices:
 * - Test features, not implementation details
 * - Use real objects, no mocks
 * - Test observable behavior
 */

import { describe, test, expect } from 'bun:test'
import {
  extractProjectName,
  calculateProjectStats,
  findCurrentTask,
  findLastActive,
  aggregateUsageStats,
  getProjectActivityStatus,
} from '../web/client/src/types/project.js'
import {
  aggregateProjects,
  calculateOverviewStats,
} from '../web/client/src/hooks/useProjects.js'
import type { Session, SessionStatus, UsageStats } from '../web/client/src/types/session.js'
import type { ProjectInfo, ProjectStats } from '../web/client/src/types/project.js'

// ============================================================================
// Test Data: Real session structures (not mocks)
// ============================================================================

/** Creates a valid Session with realistic values */
function createSession(overrides: Partial<Session> = {}): Session {
  const now = new Date().toISOString()
  return {
    id: 'test-id-' + Math.random().toString(36).slice(2),
    sessionId: 'session-' + Math.random().toString(36).slice(2),
    client: 'claude',
    directory: process.cwd(),
    status: 'running' as SessionStatus,
    title: 'Test Session',
    initialPrompt: 'Help me with something',
    lastActiveAt: now,
    toolCount: 5,
    messageCount: 10,
    createdAt: now,
    updatedAt: now,
    ...overrides,
  }
}

/** Creates a session with usage stats */
function createSessionWithUsage(
  overrides: Partial<Session> = {},
  usage: Partial<UsageStats> = {}
): Session {
  return createSession({
    ...overrides,
    usageStats: {
      totalInputTokens: 1000,
      totalOutputTokens: 500,
      totalTokens: 1500,
      totalCost: 0.05,
      apiCalls: 3,
      ...usage,
    },
  })
}

/** Creates a date string representing minutes ago from now */
function minutesAgo(minutes: number): string {
  return new Date(Date.now() - minutes * 60 * 1000).toISOString()
}

// ============================================================================
// Feature: Extract Project Name from Path
// ============================================================================

describe('Extract Project Name', () => {
  describe('when path is a standard directory', () => {
    test('extracts last segment from absolute path', () => {
      expect(extractProjectName('/Users/dev/projects/my-app')).toBe('my-app')
      expect(extractProjectName('/home/user/code/keepline')).toBe('keepline')
    })

    test('extracts name from deeply nested path', () => {
      expect(extractProjectName('/Users/dev/work/client/frontend/react-app')).toBe('react-app')
    })

    test('handles path with trailing slash', () => {
      expect(extractProjectName('/Users/dev/projects/my-app/')).toBe('my-app')
    })
  })

  describe('when path is unusual', () => {
    test('returns original path if empty', () => {
      expect(extractProjectName('')).toBe('')
    })

    test('handles root path', () => {
      expect(extractProjectName('/')).toBe('/')
    })

    test('handles single segment path', () => {
      expect(extractProjectName('/projects')).toBe('projects')
    })
  })
})

// ============================================================================
// Feature: Calculate Project Stats from Sessions
// ============================================================================

describe('Calculate Project Stats', () => {
  describe('when sessions have various statuses', () => {
    test('counts each status correctly', () => {
      const sessions = [
        createSession({ status: 'running' }),
        createSession({ status: 'running' }),
        createSession({ status: 'waiting' }),
        createSession({ status: 'idle' }),
        createSession({ status: 'lost' }),
        createSession({ status: 'completed' }),
      ]

      const stats = calculateProjectStats(sessions)

      expect(stats.running).toBe(2)
      expect(stats.waiting).toBe(1)
      expect(stats.idle).toBe(1)
      expect(stats.lost).toBe(1)
      expect(stats.completed).toBe(1)
      expect(stats.total).toBe(6)
    })

    test('handles empty sessions array', () => {
      const stats = calculateProjectStats([])

      expect(stats.running).toBe(0)
      expect(stats.waiting).toBe(0)
      expect(stats.idle).toBe(0)
      expect(stats.lost).toBe(0)
      expect(stats.completed).toBe(0)
      expect(stats.total).toBe(0)
    })

    test('handles single session', () => {
      const sessions = [createSession({ status: 'waiting' })]
      const stats = calculateProjectStats(sessions)

      expect(stats.waiting).toBe(1)
      expect(stats.total).toBe(1)
    })
  })
})

// ============================================================================
// Feature: Find Current Task from Sessions
// ============================================================================

describe('Find Current Task', () => {
  describe('when there are active sessions', () => {
    test('returns title of most recently active running session', () => {
      const sessions = [
        createSession({
          status: 'running',
          title: 'Old Task',
          lastActiveAt: minutesAgo(10),
        }),
        createSession({
          status: 'running',
          title: 'Current Task',
          lastActiveAt: minutesAgo(1),
        }),
        createSession({
          status: 'idle',
          title: 'Idle Task',
          lastActiveAt: minutesAgo(0),
        }),
      ]

      expect(findCurrentTask(sessions)).toBe('Current Task')
    })

    test('returns title of waiting session if no running', () => {
      const sessions = [
        createSession({
          status: 'waiting',
          title: 'Waiting Task',
          lastActiveAt: minutesAgo(5),
        }),
        createSession({
          status: 'idle',
          title: 'Idle Task',
          lastActiveAt: minutesAgo(1),
        }),
      ]

      expect(findCurrentTask(sessions)).toBe('Waiting Task')
    })
  })

  describe('when no active sessions exist', () => {
    test('returns title of most recent session', () => {
      const sessions = [
        createSession({
          status: 'completed',
          title: 'Old Completed',
          lastActiveAt: minutesAgo(60),
        }),
        createSession({
          status: 'idle',
          title: 'Recent Idle',
          lastActiveAt: minutesAgo(5),
        }),
      ]

      expect(findCurrentTask(sessions)).toBe('Recent Idle')
    })

    test('falls back to initialPrompt if no title', () => {
      const sessions = [
        createSession({
          status: 'idle',
          title: '',
          initialPrompt: 'Help me refactor this code',
          lastActiveAt: minutesAgo(5),
        }),
      ]

      expect(findCurrentTask(sessions)).toBe('Help me refactor this code')
    })
  })

  describe('edge cases', () => {
    test('returns undefined for empty sessions', () => {
      expect(findCurrentTask([])).toBeUndefined()
    })
  })
})

// ============================================================================
// Feature: Find Last Active Timestamp
// ============================================================================

describe('Find Last Active', () => {
  test('returns most recent lastActiveAt timestamp', () => {
    const recentTime = minutesAgo(1)
    const sessions = [
      createSession({ lastActiveAt: minutesAgo(60) }),
      createSession({ lastActiveAt: recentTime }),
      createSession({ lastActiveAt: minutesAgo(30) }),
    ]

    expect(findLastActive(sessions)).toBe(recentTime)
  })

  test('returns current timestamp for empty sessions', () => {
    const before = Date.now()
    const result = findLastActive([])
    const after = Date.now()

    const resultTime = new Date(result).getTime()
    expect(resultTime).toBeGreaterThanOrEqual(before)
    expect(resultTime).toBeLessThanOrEqual(after)
  })
})

// ============================================================================
// Feature: Aggregate Usage Stats
// ============================================================================

describe('Aggregate Usage Stats', () => {
  describe('when sessions have usage stats', () => {
    test('sums all usage values correctly', () => {
      const sessions = [
        createSessionWithUsage({}, {
          totalInputTokens: 1000,
          totalOutputTokens: 500,
          totalTokens: 1500,
          totalCost: 0.05,
          apiCalls: 3,
        }),
        createSessionWithUsage({}, {
          totalInputTokens: 2000,
          totalOutputTokens: 1000,
          totalTokens: 3000,
          totalCost: 0.10,
          apiCalls: 5,
        }),
      ]

      const aggregated = aggregateUsageStats(sessions)

      expect(aggregated).toBeDefined()
      expect(aggregated!.totalInputTokens).toBe(3000)
      expect(aggregated!.totalOutputTokens).toBe(1500)
      expect(aggregated!.totalTokens).toBe(4500)
      expect(aggregated!.totalCost).toBeCloseTo(0.15, 4)
      expect(aggregated!.apiCalls).toBe(8)
    })
  })

  describe('when no sessions have usage stats', () => {
    test('returns undefined', () => {
      const sessions = [
        createSession({ usageStats: undefined }),
        createSession({ usageStats: undefined }),
      ]

      expect(aggregateUsageStats(sessions)).toBeUndefined()
    })
  })

  describe('when mixed sessions', () => {
    test('only aggregates sessions with stats', () => {
      const sessions = [
        createSessionWithUsage({}, { totalCost: 0.05 }),
        createSession({ usageStats: undefined }),
        createSessionWithUsage({}, { totalCost: 0.10 }),
      ]

      const aggregated = aggregateUsageStats(sessions)

      expect(aggregated).toBeDefined()
      expect(aggregated!.totalCost).toBeCloseTo(0.15, 4)
    })
  })
})

// ============================================================================
// Feature: Project Activity Status
// ============================================================================

describe('Project Activity Status', () => {
  test('returns "active" when running sessions exist', () => {
    const stats: ProjectStats = {
      running: 1, waiting: 0, idle: 2, lost: 0, completed: 3, total: 6
    }
    expect(getProjectActivityStatus(stats)).toBe('active')
  })

  test('returns "active" when waiting sessions exist', () => {
    const stats: ProjectStats = {
      running: 0, waiting: 2, idle: 1, lost: 0, completed: 0, total: 3
    }
    expect(getProjectActivityStatus(stats)).toBe('active')
  })

  test('returns "idle" when only idle sessions exist', () => {
    const stats: ProjectStats = {
      running: 0, waiting: 0, idle: 3, lost: 0, completed: 2, total: 5
    }
    expect(getProjectActivityStatus(stats)).toBe('idle')
  })

  test('returns "inactive" when only lost/completed sessions', () => {
    const stats: ProjectStats = {
      running: 0, waiting: 0, idle: 0, lost: 1, completed: 4, total: 5
    }
    expect(getProjectActivityStatus(stats)).toBe('inactive')
  })
})

// ============================================================================
// Feature: Aggregate Projects from Sessions
// ============================================================================

describe('Aggregate Projects', () => {
  describe('when sessions belong to different directories', () => {
    test('groups sessions by directory', () => {
      const sessions = [
        createSession({ directory: '/project-a', status: 'running' }),
        createSession({ directory: '/project-a', status: 'idle' }),
        createSession({ directory: '/project-b', status: 'waiting' }),
      ]

      const projects = aggregateProjects(sessions)

      expect(projects).toHaveLength(2)

      const projectA = projects.find(p => p.path === '/project-a')
      const projectB = projects.find(p => p.path === '/project-b')

      expect(projectA).toBeDefined()
      expect(projectA!.sessions).toHaveLength(2)
      expect(projectA!.name).toBe('project-a')

      expect(projectB).toBeDefined()
      expect(projectB!.sessions).toHaveLength(1)
      expect(projectB!.name).toBe('project-b')
    })

    test('sorts projects by last activity (most recent first)', () => {
      const sessions = [
        createSession({
          directory: '/old-project',
          lastActiveAt: minutesAgo(60),
        }),
        createSession({
          directory: '/recent-project',
          lastActiveAt: minutesAgo(1),
        }),
      ]

      const projects = aggregateProjects(sessions)

      expect(projects[0].path).toBe('/recent-project')
      expect(projects[1].path).toBe('/old-project')
    })
  })

  describe('when sessions have no directory', () => {
    test('groups under "Unknown"', () => {
      const sessions = [
        createSession({ directory: '' }),
        createSession({ directory: '' }),
      ]

      const projects = aggregateProjects(sessions)

      expect(projects).toHaveLength(1)
      expect(projects[0].path).toBe('Unknown')
    })
  })

  describe('when no sessions exist', () => {
    test('returns empty array', () => {
      expect(aggregateProjects([])).toHaveLength(0)
    })
  })
})

// ============================================================================
// Feature: Calculate Overview Stats
// ============================================================================

describe('Calculate Overview Stats', () => {
  test('counts active and idle projects correctly', () => {
    const projects: ProjectInfo[] = [
      {
        path: '/active-1',
        name: 'active-1',
        sessions: [],
        stats: { running: 1, waiting: 0, idle: 0, lost: 0, completed: 0, total: 1 },
        lastActiveAt: new Date().toISOString(),
      },
      {
        path: '/active-2',
        name: 'active-2',
        sessions: [],
        stats: { running: 0, waiting: 1, idle: 0, lost: 0, completed: 0, total: 1 },
        lastActiveAt: new Date().toISOString(),
      },
      {
        path: '/idle-1',
        name: 'idle-1',
        sessions: [],
        stats: { running: 0, waiting: 0, idle: 2, lost: 0, completed: 0, total: 2 },
        lastActiveAt: new Date().toISOString(),
      },
    ]

    const stats = calculateOverviewStats(projects)

    expect(stats.total).toBe(3)
    expect(stats.active).toBe(2)
    expect(stats.idle).toBe(1)
  })

  test('handles empty projects array', () => {
    const stats = calculateOverviewStats([])

    expect(stats.total).toBe(0)
    expect(stats.active).toBe(0)
    expect(stats.idle).toBe(0)
  })
})
