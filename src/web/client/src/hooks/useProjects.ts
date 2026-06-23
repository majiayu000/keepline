import { useCallback, useEffect, useRef, useState } from 'react'
import { api } from '@/services/api'
import type { ProjectInfo, ProjectOverviewStats } from '@/types/project'

export interface UseProjectsReturn {
  /** List of projects, sorted by last activity */
  projects: ProjectInfo[]
  /** Overview statistics */
  stats: ProjectOverviewStats
  /** Whether any projects exist */
  hasProjects: boolean
  /** Whether projects are loading */
  loading: boolean
  /** Last project API error */
  error: string | null
  /** Refresh project summaries */
  refresh: () => Promise<void>
}

/**
 * Loads project summaries from the backend project API.
 *
 * The optional refreshKey lets the sessions websocket drive a project refresh
 * without deriving projects from a paginated sessions list.
 */
export function useProjects(_token: string, refreshKey = 0): UseProjectsReturn {
  const [projects, setProjects] = useState<ProjectInfo[]>([])
  const [stats, setStats] = useState<ProjectOverviewStats>({ total: 0, active: 0, idle: 0 })
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const mountedRef = useRef(true)
  const refreshRequestIdRef = useRef(0)

  const refresh = useCallback(async () => {
    const requestId = ++refreshRequestIdRef.current
    const response = await api.fetchProjects('basic')
    if (!mountedRef.current) return
    if (requestId !== refreshRequestIdRef.current) return

    if (response.success && response.data) {
      setProjects(response.data.projects)
      setStats(response.data.stats)
      setError(null)
    } else {
      setError(response.error || 'Failed to load projects')
    }
    setLoading(false)
  }, [])

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  useEffect(() => {
    setLoading(true)
    refresh()
  }, [refresh, refreshKey])

  return {
    projects,
    stats,
    hasProjects: projects.length > 0,
    loading,
    error,
    refresh,
  }
}
