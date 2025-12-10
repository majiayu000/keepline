import { useState, useEffect, useCallback, useRef } from 'react'
import { api } from '@/services/api'
import { REFRESH_INTERVAL_MS } from '@/constants'
import type { Session, SessionStats, SessionDetailsData } from '@/types'

interface UseSessionsReturn {
  sessions: Session[]
  stats: SessionStats | null
  loading: boolean
  syncing: boolean
  error: string | null
  refresh: () => Promise<void>
  sync: () => Promise<boolean>
  recoverSession: (sessionId: string) => Promise<boolean>
  stopSession: (sessionId: string) => Promise<boolean>
  completeSession: (sessionId: string) => Promise<boolean>
  // Lazy loading
  getSessionDetails: (sessionId: string) => SessionDetailsData | undefined
  loadSessionDetails: (sessionId: string) => Promise<SessionDetailsData | null>
  isLoadingDetails: (sessionId: string) => boolean
}

export function useSessions(): UseSessionsReturn {
  const [sessions, setSessions] = useState<Session[]>([])
  const [stats, setStats] = useState<SessionStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [syncing, setSyncing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const intervalRef = useRef<number | null>(null)
  const mountedRef = useRef(true)

  // Details cache for lazy loading
  const [detailsCache, setDetailsCache] = useState<Map<string, SessionDetailsData>>(new Map())
  const [loadingDetails, setLoadingDetails] = useState<Set<string>>(new Set())

  // Use ref to avoid stale closures in interval
  const loadSessions = useCallback(async () => {
    // Use 'basic' mode for faster loading
    const response = await api.fetchSessions('basic')

    // Only update state if component is still mounted
    if (!mountedRef.current) return

    if (response.success && response.data) {
      setSessions(response.data.sessions)
      setStats(response.data.stats)
      setError(null)
    } else {
      setError(response.error || 'Failed to load sessions')
    }
  }, [])

  const refresh = useCallback(async () => {
    setLoading(true)
    await loadSessions()
    if (mountedRef.current) {
      setLoading(false)
    }
  }, [loadSessions])

  const sync = useCallback(async () => {
    setSyncing(true)
    const response = await api.syncSessions()
    const success = response.success
    if (success) {
      await loadSessions()
    }
    if (mountedRef.current) {
      setSyncing(false)
    }
    return success
  }, [loadSessions])

  const recoverSession = useCallback(async (sessionId: string) => {
    const response = await api.recoverSession(sessionId, { method: 'resume' })
    if (response.success) {
      await loadSessions()
    }
    return response.success
  }, [loadSessions])

  const stopSession = useCallback(async (sessionId: string) => {
    const response = await api.stopSession(sessionId, {})
    if (response.success) {
      await loadSessions()
    }
    return response.success
  }, [loadSessions])

  const completeSession = useCallback(async (sessionId: string) => {
    const response = await api.completeSession(sessionId)
    if (response.success) {
      await loadSessions()
    }
    return response.success
  }, [loadSessions])

  // Lazy load session details
  const loadSessionDetails = useCallback(async (sessionId: string): Promise<SessionDetailsData | null> => {
    // Return cached if available
    const cached = detailsCache.get(sessionId)
    if (cached) return cached

    // Skip if already loading
    if (loadingDetails.has(sessionId)) return null

    // Mark as loading
    setLoadingDetails(prev => new Set(prev).add(sessionId))

    const response = await api.fetchSessionDetails(sessionId)

    if (!mountedRef.current) return null

    // Remove from loading set
    setLoadingDetails(prev => {
      const next = new Set(prev)
      next.delete(sessionId)
      return next
    })

    if (response.success && response.data) {
      // Cache the result
      setDetailsCache(prev => new Map(prev).set(sessionId, response.data!))
      return response.data
    }

    return null
  }, [detailsCache, loadingDetails])

  // Get cached details (synchronous)
  const getSessionDetails = useCallback((sessionId: string): SessionDetailsData | undefined => {
    return detailsCache.get(sessionId)
  }, [detailsCache])

  // Check if details are loading
  const isLoadingDetails = useCallback((sessionId: string): boolean => {
    return loadingDetails.has(sessionId)
  }, [loadingDetails])

  // Initial load - run once on mount
  useEffect(() => {
    mountedRef.current = true
    refresh()

    return () => {
      mountedRef.current = false
    }
  }, []) // Empty deps - only run on mount

  // Auto-refresh interval
  useEffect(() => {
    intervalRef.current = window.setInterval(() => {
      loadSessions()
    }, REFRESH_INTERVAL_MS)

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current)
      }
    }
  }, [loadSessions])

  return {
    sessions,
    stats,
    loading,
    syncing,
    error,
    refresh,
    sync,
    recoverSession,
    stopSession,
    completeSession,
    // Lazy loading
    getSessionDetails,
    loadSessionDetails,
    isLoadingDetails,
  }
}
