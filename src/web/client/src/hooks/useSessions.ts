import { useState, useEffect, useCallback, useRef } from 'react'
import { api } from '@/services/api'
import { REFRESH_INTERVAL_MS } from '@/constants'
import { useWebSocket, type WebSocketMessage } from './useWebSocket'
import type { Session, SessionStats, SessionFullData, PaginationInfo, TerminalApp } from '@/types'

export type ConnectionStatus = 'polling' | 'realtime' | 'disconnected'

// Number of sessions to load per page
const PAGE_SIZE = 50
const SNAPSHOT_PAGE_SIZE = 100

async function fetchAllBasicSessionsSnapshot(): Promise<{ sessions: Session[]; error: string | null }> {
  const sessions: Session[] = []
  let offset = 0

  while (true) {
    const response = await api.fetchSessions('basic', {
      limit: SNAPSHOT_PAGE_SIZE,
      offset,
      skipSync: true,
    })

    if (!response.success || !response.data) {
      return {
        sessions: [],
        error: response.error || 'Failed to load unfiltered sessions',
      }
    }

    const page = response.data.sessions
    sessions.push(...page)

    if (!response.data.pagination?.hasMore) {
      return { sessions, error: null }
    }

    if (page.length === 0) {
      return {
        sessions: [],
        error: 'Failed to load complete unfiltered sessions',
      }
    }

    offset += page.length
  }
}

function getSessionVersionSignature(sessions: Session[]): string {
  return sessions
    .map(session => [
      session.sessionId,
      session.client,
      session.directory,
      session.status,
      session.title,
      session.lastActiveAt,
      session.completedAt || '',
      session.toolCount,
      session.messageCount,
      session.updatedAt,
    ].join(':'))
    .join('|')
}

interface UseSessionsReturn {
  sessions: Session[]
  allSessions: Session[]
  stats: SessionStats | null
  loading: boolean
  syncing: boolean
  error: string | null
  refresh: () => Promise<void>
  sync: () => Promise<boolean>
  recoverSession: (sessionId: string, terminalApp?: TerminalApp) => Promise<boolean>
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
  // Incremented when session data changes
  version: number
}

interface UseSessionsOptions {
  projectRoot?: string
}

export function useSessions(token: string, options: UseSessionsOptions = {}): UseSessionsReturn {
  const [sessions, setSessions] = useState<Session[]>([])
  const [allSessions, setAllSessions] = useState<Session[]>([])
  const [stats, setStats] = useState<SessionStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [syncing, setSyncing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>('polling')
  const [pagination, setPagination] = useState<PaginationInfo | null>(null)
  const [loadingMore, setLoadingMore] = useState(false)
  const [version, setVersion] = useState(0)
  const intervalRef = useRef<number | null>(null)
  const mountedRef = useRef(true)
  const wsConnectedRef = useRef(false)
  const projectRootRef = useRef<string | undefined>(options.projectRoot)
  const loadRequestIdRef = useRef(0)
  const versionSignatureRef = useRef('')

  // Full data cache for lazy loading - use refs to avoid re-render loops
  // Now caches combined data from /full endpoint (details + tools + subagents)
  const fullDataCacheRef = useRef<Map<string, SessionFullData>>(new Map())
  const loadingFullRef = useRef<Set<string>>(new Set())
  // Trigger re-render when cache updates
  const [, forceUpdate] = useState(0)

  const bumpVersionIfChanged = useCallback((snapshot: Session[]) => {
    const nextSignature = getSessionVersionSignature(snapshot)
    if (nextSignature === versionSignatureRef.current) return
    versionSignatureRef.current = nextSignature
    setVersion(v => v + 1)
  }, [])

  useEffect(() => {
    projectRootRef.current = options.projectRoot
  }, [options.projectRoot])

  // Handle WebSocket messages
  const handleWebSocketMessage = useCallback((message: WebSocketMessage) => {
    if (!mountedRef.current) return

    if (message.type === 'sessions:update' && message.data) {
      const data = message.data as { sessions: Session[]; stats: SessionStats }
      setAllSessions(data.sessions)
      bumpVersionIfChanged(data.sessions)
      if (projectRootRef.current) {
        loadSessionsRef.current()
        return
      }
      setSessions(data.sessions)
      setStats(data.stats)
      setPagination(null)
      setError(null)
    }
  }, [bumpVersionIfChanged])

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

  // Use ref for loadSessions to avoid stale closures
  const loadSessionsRef = useRef<() => Promise<void>>(() => Promise.resolve())

  // Use ref to avoid stale closures in interval
  const loadSessions = useCallback(async () => {
    const requestId = ++loadRequestIdRef.current
    const requestedProjectRoot = options.projectRoot

    // Use 'basic' mode for faster loading, with pagination
    const response = await api.fetchSessions('basic', {
      limit: PAGE_SIZE,
      projectRoot: requestedProjectRoot,
    })

    // Only update state if component is still mounted
    if (!mountedRef.current) return
    if (requestId !== loadRequestIdRef.current) return
    if (requestedProjectRoot !== projectRootRef.current) return

    if (response.success && response.data) {
      setSessions(response.data.sessions)
      if (requestedProjectRoot) {
        fetchAllBasicSessionsSnapshot().then(unfilteredResponse => {
          if (!mountedRef.current) return
          if (requestId !== loadRequestIdRef.current) return
          if (requestedProjectRoot !== projectRootRef.current) return

          if (!unfilteredResponse.error) {
            setAllSessions(unfilteredResponse.sessions)
            bumpVersionIfChanged(unfilteredResponse.sessions)
          } else {
            setError(unfilteredResponse.error)
          }
        })
      } else {
        if (!response.data.pagination?.hasMore) {
          setAllSessions(response.data.sessions)
          bumpVersionIfChanged(response.data.sessions)
        } else {
          fetchAllBasicSessionsSnapshot().then(unfilteredResponse => {
            if (!mountedRef.current) return
            if (requestId !== loadRequestIdRef.current) return
            if (requestedProjectRoot !== projectRootRef.current) return

            if (!unfilteredResponse.error) {
              setAllSessions(unfilteredResponse.sessions)
              bumpVersionIfChanged(unfilteredResponse.sessions)
            } else {
              setError(unfilteredResponse.error)
            }
          })
        }
      }
      setStats(response.data.stats)
      setPagination(response.data.pagination || null)
      setError(null)
    } else {
      setError(response.error || 'Failed to load sessions')
    }
  }, [bumpVersionIfChanged, options.projectRoot])

  // Load more sessions (pagination)
  const loadMore = useCallback(async () => {
    if (!pagination?.hasMore || loadingMore) return

    const requestedProjectRoot = options.projectRoot
    setLoadingMore(true)
    const response = await api.fetchSessions('basic', {
      limit: PAGE_SIZE,
      offset: sessions.length,
      skipSync: true, // Don't trigger sync for pagination requests
      projectRoot: requestedProjectRoot,
    })

    if (!mountedRef.current) return
    if (requestedProjectRoot !== projectRootRef.current) {
      setLoadingMore(false)
      return
    }

    if (response.success && response.data) {
      // Append new sessions to existing list
      const nextSessions = response.data.sessions
      setSessions(prev => [...prev, ...nextSessions])
      setPagination(response.data.pagination || null)
    }
    setLoadingMore(false)
  }, [pagination, loadingMore, sessions.length, options.projectRoot])

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
  }, []) // Empty deps - only run on mount

  useEffect(() => {
    refresh()
  }, [refresh])

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
    version,
  }
}
