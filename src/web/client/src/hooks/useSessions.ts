import { useState, useEffect, useCallback, useRef } from 'react'
import { api } from '@/services/api'
import { REFRESH_INTERVAL_MS } from '@/constants'
import { useWebSocket, type WebSocketMessage } from './useWebSocket'
import type { Session, SessionStats, SessionFullData, PaginationInfo, TerminalApp, SessionStatus } from '@/types'

export type ConnectionStatus = 'polling' | 'realtime' | 'disconnected'
export interface SessionQueryOptions {
  searchQuery?: string
  statusFilters?: Set<SessionStatus>
}

export interface SessionActionResult {
  success: boolean
  error?: string
  command?: string
}

// Number of sessions to load per page
const PAGE_SIZE = 50

interface UseSessionsReturn {
  sessions: Session[]
  allSessions: Session[]
  stats: SessionStats | null
  loading: boolean
  syncing: boolean
  error: string | null
  refresh: () => Promise<void>
  sync: () => Promise<boolean>
  recoverSession: (sessionId: string, terminalApp?: TerminalApp) => Promise<SessionActionResult>
  stopSession: (sessionId: string) => Promise<boolean>
  completeSession: (sessionId: string) => Promise<boolean>
  // Lazy loading - now uses combined /full endpoint (1 request instead of 3)
  getSessionFull: (sessionId: string) => SessionFullData | undefined
  loadSessionFull: (sessionId: string) => Promise<SessionFullData | null>
  isLoadingFull: (sessionId: string) => boolean
  // Pagination
  pagination: PaginationInfo | null
  loadMore: () => Promise<void>
  loadingMore: boolean
  // WebSocket status
  connectionStatus: ConnectionStatus
}

export function useSessions(token: string, options: SessionQueryOptions = {}): UseSessionsReturn {
  const searchQuery = options.searchQuery?.trim() ?? ''
  const statusFilterValues = Array.from(options.statusFilters ?? []).sort()
  const statusFilterKey = statusFilterValues.join(',')
  const listQueryKey = `${searchQuery}\u0000${statusFilterKey}`
  const hasServerFilters = searchQuery.length > 0 || statusFilterValues.length > 0
  const [sessions, setSessions] = useState<Session[]>([])
  const [allSessions, setAllSessions] = useState<Session[]>([])
  const [stats, setStats] = useState<SessionStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [syncing, setSyncing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>('polling')
  const [pagination, setPagination] = useState<PaginationInfo | null>(null)
  const [loadingMore, setLoadingMore] = useState(false)
  const intervalRef = useRef<number | null>(null)
  const mountedRef = useRef(true)
  const wsConnectedRef = useRef(false)
  const loadSessionsRef = useRef<() => Promise<void>>(() => Promise.resolve())
  const listRequestSeqRef = useRef(0)
  const loadMoreRequestSeqRef = useRef(0)
  const listQueryKeyRef = useRef(listQueryKey)
  const sessionsLengthRef = useRef(0)
  listQueryKeyRef.current = listQueryKey
  sessionsLengthRef.current = sessions.length

  // Full data cache for lazy loading - use refs to avoid re-render loops
  // Now caches combined data from /full endpoint (details + tools + subagents)
  const fullDataCacheRef = useRef<Map<string, SessionFullData>>(new Map())
  const loadingFullRef = useRef<Set<string>>(new Set())
  // Trigger re-render when cache updates
  const [, forceUpdate] = useState(0)

  // Handle WebSocket messages
  const handleWebSocketMessage = useCallback((message: WebSocketMessage) => {
    if (!mountedRef.current) return

    if (message.type === 'sessions:update' && message.data) {
      const data = message.data as { sessions: Session[]; stats: SessionStats }
      setAllSessions(data.sessions)
      if (hasServerFilters) {
        loadSessionsRef.current()
        return
      }
      setSessions(data.sessions)
      setStats(data.stats)
      setError(null)
    }
  }, [hasServerFilters])

  // WebSocket connection
  useWebSocket({
    token,
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

  // Use ref to avoid stale closures in interval
  const loadSessions = useCallback(async () => {
    const requestSeq = ++listRequestSeqRef.current
    // Use 'basic' mode for faster loading, with pagination
    const [response, unfilteredResponse] = await Promise.all([
      api.fetchSessions('basic', {
        limit: PAGE_SIZE,
        query: searchQuery,
        status: statusFilterValues,
      }),
      hasServerFilters
        ? api.fetchSessions('basic', {
          limit: PAGE_SIZE,
          skipSync: true,
        })
        : Promise.resolve(null),
    ])

    // Only update state if component is still mounted
    if (!mountedRef.current || requestSeq !== listRequestSeqRef.current) return

    if (response.success && response.data) {
      setSessions(response.data.sessions)
      let unfilteredError: string | null = null
      if (hasServerFilters) {
        if (unfilteredResponse?.success && unfilteredResponse.data) {
          setAllSessions(unfilteredResponse.data.sessions)
        } else {
          unfilteredError = unfilteredResponse?.error || 'Failed to load unfiltered sessions'
        }
      } else {
        setAllSessions(response.data.sessions)
      }
      setStats(response.data.stats)
      setPagination(response.data.pagination || null)
      setError(unfilteredError)
    } else {
      setError(response.error || 'Failed to load sessions')
    }
  }, [searchQuery, statusFilterKey, hasServerFilters])

  // Load more sessions (pagination)
  const loadMore = useCallback(async () => {
    if (!pagination?.hasMore || loadingMore) return

    setLoadingMore(true)
    const requestSeq = listRequestSeqRef.current
    const loadMoreSeq = ++loadMoreRequestSeqRef.current
    const requestQueryKey = listQueryKey
    const requestOffset = sessions.length
    const response = await api.fetchSessions('basic', {
      limit: PAGE_SIZE,
      offset: requestOffset,
      skipSync: true, // Don't trigger sync for pagination requests
      query: searchQuery,
      status: statusFilterValues,
    })

    if (
      !mountedRef.current ||
      loadMoreSeq !== loadMoreRequestSeqRef.current ||
      requestSeq !== listRequestSeqRef.current ||
      requestQueryKey !== listQueryKeyRef.current ||
      requestOffset !== sessionsLengthRef.current
    ) {
      if (mountedRef.current && loadMoreSeq === loadMoreRequestSeqRef.current) {
        setLoadingMore(false)
      }
      return
    }

    if (response.success && response.data) {
      const nextSessions = response.data.sessions
      setSessions(prev => [...prev, ...nextSessions])
      if (!hasServerFilters) {
        setAllSessions(prev => [...prev, ...nextSessions])
      }
      setPagination(response.data.pagination || null)
    }
    setLoadingMore(false)
  }, [pagination, loadingMore, sessions.length, searchQuery, statusFilterKey, listQueryKey, hasServerFilters])

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

  const recoverSession = useCallback(async (sessionId: string, terminalApp?: TerminalApp) => {
    const response = await api.recoverSession(sessionId, {
      terminalApp: terminalApp ?? 'auto',
    })
    if (response.success) {
      await loadSessions()
    }
    const command = response.data?.command
    return {
      success: response.success,
      error: command && response.error
        ? `${response.error}. Run manually: ${command}`
        : response.error,
      command,
    }
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

  // Lazy load full session data (details + tools + subagents) - 1 request instead of 3
  const loadSessionFull = useCallback(async (sessionId: string): Promise<SessionFullData | null> => {
    // Return cached if available
    const cached = fullDataCacheRef.current.get(sessionId)
    if (cached) return cached

    // Skip if already loading
    if (loadingFullRef.current.has(sessionId)) return null

    // Mark as loading
    loadingFullRef.current.add(sessionId)
    forceUpdate(n => n + 1)

    const response = await api.fetchSessionFull(sessionId)

    if (!mountedRef.current) return null

    // Remove from loading set
    loadingFullRef.current.delete(sessionId)

    if (response.success && response.data) {
      // Cache the result
      fullDataCacheRef.current.set(sessionId, response.data)
      forceUpdate(n => n + 1)
      return response.data
    }

    forceUpdate(n => n + 1)
    return null
  }, []) // No dependencies - uses refs

  // Get cached full data (synchronous)
  const getSessionFull = useCallback((sessionId: string): SessionFullData | undefined => {
    return fullDataCacheRef.current.get(sessionId)
  }, []) // No dependencies - uses ref

  // Check if full data is loading
  const isLoadingFull = useCallback((sessionId: string): boolean => {
    return loadingFullRef.current.has(sessionId)
  }, []) // No dependencies - uses ref

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  useEffect(() => {
    let cancelled = false
    const timeoutId = window.setTimeout(async () => {
      setLoading(true)
      await loadSessions()
      if (!cancelled && mountedRef.current) {
        setLoading(false)
      }
    }, searchQuery ? 250 : 0)

    return () => {
      cancelled = true
      clearTimeout(timeoutId)
    }
  }, [loadSessions, searchQuery])

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
    allSessions,
    stats,
    loading,
    syncing,
    error,
    refresh,
    sync,
    recoverSession,
    stopSession,
    completeSession,
    // Lazy loading - combined endpoint (1 request instead of 3)
    getSessionFull,
    loadSessionFull,
    isLoadingFull,
    // Pagination
    pagination,
    loadMore,
    loadingMore,
    // WebSocket status
    connectionStatus,
  }
}
