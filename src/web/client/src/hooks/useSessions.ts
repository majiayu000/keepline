import { useState, useEffect, useCallback, useRef } from 'react'
import { api } from '@/services/api'
import { REFRESH_INTERVAL_MS } from '@/constants'
import { useWebSocket, type WebSocketMessage } from './useWebSocket'
import type { Session, SessionStats, SessionDetailsData } from '@/types'

export type ConnectionStatus = 'polling' | 'realtime' | 'disconnected'

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
  // WebSocket status
  connectionStatus: ConnectionStatus
}

export function useSessions(): UseSessionsReturn {
  const [sessions, setSessions] = useState<Session[]>([])
  const [stats, setStats] = useState<SessionStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [syncing, setSyncing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>('polling')
  const intervalRef = useRef<number | null>(null)
  const mountedRef = useRef(true)
  const wsConnectedRef = useRef(false)

  // Details cache for lazy loading - use refs to avoid re-render loops
  const detailsCacheRef = useRef<Map<string, SessionDetailsData>>(new Map())
  const loadingDetailsRef = useRef<Set<string>>(new Set())
  // Trigger re-render when cache updates
  const [, forceUpdate] = useState(0)

  // Handle WebSocket messages
  const handleWebSocketMessage = useCallback((message: WebSocketMessage) => {
    if (!mountedRef.current) return

    if (message.type === 'sessions:update' && message.data) {
      const data = message.data as { sessions: Session[]; stats: SessionStats }
      setSessions(data.sessions)
      setStats(data.stats)
      setError(null)
    }
  }, [])

  // WebSocket connection
  useWebSocket({
    onMessage: handleWebSocketMessage,
    onConnect: () => {
      wsConnectedRef.current = true
      setConnectionStatus('realtime')
      // Clear polling interval when WebSocket is connected
      if (intervalRef.current) {
        clearInterval(intervalRef.current)
        intervalRef.current = null
      }
    },
    onDisconnect: () => {
      wsConnectedRef.current = false
      setConnectionStatus('polling')
      // Restart polling when WebSocket disconnects
      if (!intervalRef.current) {
        intervalRef.current = window.setInterval(() => {
          loadSessionsRef.current()
        }, REFRESH_INTERVAL_MS)
      }
    },
  })

  // Use ref for loadSessions to avoid stale closures
  const loadSessionsRef = useRef<() => Promise<void>>(() => Promise.resolve())

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

  // Keep loadSessionsRef updated
  useEffect(() => {
    loadSessionsRef.current = loadSessions
  }, [loadSessions])

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

  // Lazy load session details - stable reference using refs
  const loadSessionDetails = useCallback(async (sessionId: string): Promise<SessionDetailsData | null> => {
    // Return cached if available
    const cached = detailsCacheRef.current.get(sessionId)
    if (cached) return cached

    // Skip if already loading
    if (loadingDetailsRef.current.has(sessionId)) return null

    // Mark as loading
    loadingDetailsRef.current.add(sessionId)
    forceUpdate(n => n + 1)

    const response = await api.fetchSessionDetails(sessionId)

    if (!mountedRef.current) return null

    // Remove from loading set
    loadingDetailsRef.current.delete(sessionId)

    if (response.success && response.data) {
      // Cache the result
      detailsCacheRef.current.set(sessionId, response.data)
      forceUpdate(n => n + 1)
      return response.data
    }

    forceUpdate(n => n + 1)
    return null
  }, []) // No dependencies - uses refs

  // Get cached details (synchronous)
  const getSessionDetails = useCallback((sessionId: string): SessionDetailsData | undefined => {
    return detailsCacheRef.current.get(sessionId)
  }, []) // No dependencies - uses ref

  // Check if details are loading
  const isLoadingDetails = useCallback((sessionId: string): boolean => {
    return loadingDetailsRef.current.has(sessionId)
  }, []) // No dependencies - uses ref

  // Initial load - run once on mount
  useEffect(() => {
    mountedRef.current = true
    refresh()

    return () => {
      mountedRef.current = false
    }
  }, []) // Empty deps - only run on mount

  // Auto-refresh interval (only when WebSocket is not connected)
  useEffect(() => {
    // Only start polling if WebSocket is not connected
    if (!wsConnectedRef.current) {
      intervalRef.current = window.setInterval(() => {
        loadSessionsRef.current()
      }, REFRESH_INTERVAL_MS)
    }

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current)
        intervalRef.current = null
      }
    }
  }, []) // Remove loadSessions dependency - use ref instead

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
    // WebSocket status
    connectionStatus,
  }
}
