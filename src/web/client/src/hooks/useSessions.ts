import { useState, useEffect, useCallback, useRef } from 'react'
import { api } from '@/services/api'
import { REFRESH_INTERVAL_MS } from '@/constants'
import { useWebSocket, type WebSocketMessage } from './useWebSocket'
import type { Session, SessionStats, SessionFullData, PaginationInfo, TerminalApp, SessionStatus, RuntimeFilter } from '@/types'

export type ConnectionStatus = 'polling' | 'realtime' | 'disconnected'

export interface SessionQueryOptions {
  searchQuery?: string
  statusFilters?: Set<SessionStatus>
  runtimeFilter?: RuntimeFilter
  projectRoot?: string
  projectId?: string
}

export interface SessionActionResult {
  success: boolean
  error?: string
  command?: string
}

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
  recoverSession: (sessionId: string, terminalApp?: TerminalApp) => Promise<SessionActionResult>
  stopSession: (sessionId: string) => Promise<boolean>
  completeSession: (sessionId: string) => Promise<boolean>
  getSessionFull: (sessionId: string) => SessionFullData | undefined
  loadSessionFull: (sessionId: string) => Promise<SessionFullData | null>
  isLoadingFull: (sessionId: string) => boolean
  pagination: PaginationInfo | null
  loadMore: () => Promise<void>
  loadingMore: boolean
  connectionStatus: ConnectionStatus
  version: number
}

export function useSessions(token: string, options: SessionQueryOptions = {}): UseSessionsReturn {
  const searchQuery = options.searchQuery?.trim() ?? ''
  const statusFilterValues = Array.from(options.statusFilters ?? []).sort()
  const statusFilterKey = statusFilterValues.join(',')
  const runtimeFilter = options.runtimeFilter ?? 'all'
  const projectRoot = options.projectRoot
  const projectId = options.projectId
  const listQueryKey = `${searchQuery}\u0000${statusFilterKey}\u0000${runtimeFilter}\u0000${projectRoot ?? ''}\u0000${projectId ?? ''}`
  const hasServerFilters = searchQuery.length > 0 ||
    statusFilterValues.length > 0 ||
    runtimeFilter !== 'all' ||
    Boolean(projectRoot || projectId)

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
  const loadSessionsRef = useRef<() => Promise<void>>(() => Promise.resolve())
  const listRequestSeqRef = useRef(0)
  const loadMoreRequestSeqRef = useRef(0)
  const listQueryKeyRef = useRef(listQueryKey)
  const sessionsLengthRef = useRef(0)
  const versionSignatureRef = useRef('')

  listQueryKeyRef.current = listQueryKey
  sessionsLengthRef.current = sessions.length

  const fullDataCacheRef = useRef<Map<string, SessionFullData>>(new Map())
  const loadingFullRef = useRef<Set<string>>(new Set())
  const [, forceUpdate] = useState(0)

  const bumpVersionIfChanged = useCallback((snapshot: Session[]) => {
    const nextSignature = getSessionVersionSignature(snapshot)
    if (nextSignature === versionSignatureRef.current) return
    versionSignatureRef.current = nextSignature
    setVersion(v => v + 1)
  }, [])

  const handleWebSocketMessage = useCallback((message: WebSocketMessage) => {
    if (!mountedRef.current) return

    if (message.type === 'sessions:update' && message.data) {
      const data = message.data as { sessions: Session[]; stats: SessionStats }
      setAllSessions(data.sessions)
      bumpVersionIfChanged(data.sessions)
      if (hasServerFilters) {
        loadSessionsRef.current()
        return
      }
      setSessions(data.sessions)
      setStats(data.stats)
      setPagination(null)
      setError(null)
    }
  }, [bumpVersionIfChanged, hasServerFilters])

  useWebSocket({
    token,
    onMessage: handleWebSocketMessage,
    onConnect: () => {
      wsConnectedRef.current = true
      setConnectionStatus('realtime')
      if (intervalRef.current) {
        clearInterval(intervalRef.current)
        intervalRef.current = null
      }
    },
    onDisconnect: () => {
      wsConnectedRef.current = false
      setConnectionStatus('polling')
      if (!intervalRef.current) {
        intervalRef.current = window.setInterval(() => {
          loadSessionsRef.current()
        }, REFRESH_INTERVAL_MS)
      }
    },
  })

  const loadSessions = useCallback(async () => {
    const requestSeq = ++listRequestSeqRef.current
    const requestQueryKey = listQueryKey
    const response = await api.fetchSessions('basic', {
      limit: PAGE_SIZE,
      query: searchQuery,
      status: statusFilterValues,
      runtime: runtimeFilter,
      projectRoot,
      projectId,
    })

    if (
      !mountedRef.current ||
      requestSeq !== listRequestSeqRef.current ||
      requestQueryKey !== listQueryKeyRef.current
    ) return

    if (response.success && response.data) {
      setSessions(response.data.sessions)
      setStats(response.data.stats)
      setPagination(response.data.pagination || null)
      setError(null)

      if (!hasServerFilters && !response.data.pagination?.hasMore) {
        setAllSessions(response.data.sessions)
        bumpVersionIfChanged(response.data.sessions)
        return
      }

      fetchAllBasicSessionsSnapshot().then(unfilteredResponse => {
        if (
          !mountedRef.current ||
          requestSeq !== listRequestSeqRef.current ||
          requestQueryKey !== listQueryKeyRef.current
        ) return

        if (!unfilteredResponse.error) {
          setAllSessions(unfilteredResponse.sessions)
          bumpVersionIfChanged(unfilteredResponse.sessions)
        } else {
          setError(unfilteredResponse.error)
        }
      })
    } else {
      setError(response.error || 'Failed to load sessions')
    }
  }, [
    searchQuery,
    statusFilterKey,
    runtimeFilter,
    projectRoot,
    projectId,
    hasServerFilters,
    listQueryKey,
    bumpVersionIfChanged,
  ])

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
      skipSync: true,
      query: searchQuery,
      status: statusFilterValues,
      runtime: runtimeFilter,
      projectRoot,
      projectId,
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
      setPagination(response.data.pagination || null)
    }
    setLoadingMore(false)
  }, [
    pagination,
    loadingMore,
    sessions.length,
    searchQuery,
    statusFilterKey,
    runtimeFilter,
    projectRoot,
    projectId,
    listQueryKey,
  ])

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

  const loadSessionFull = useCallback(async (sessionId: string): Promise<SessionFullData | null> => {
    const cached = fullDataCacheRef.current.get(sessionId)
    if (cached) return cached
    if (loadingFullRef.current.has(sessionId)) return null

    loadingFullRef.current.add(sessionId)
    forceUpdate(n => n + 1)

    const response = await api.fetchSessionFull(sessionId)

    if (!mountedRef.current) return null
    loadingFullRef.current.delete(sessionId)

    if (response.success && response.data) {
      fullDataCacheRef.current.set(sessionId, response.data)
      forceUpdate(n => n + 1)
      return response.data
    }

    forceUpdate(n => n + 1)
    return null
  }, [])

  const getSessionFull = useCallback((sessionId: string): SessionFullData | undefined => {
    return fullDataCacheRef.current.get(sessionId)
  }, [])

  const isLoadingFull = useCallback((sessionId: string): boolean => {
    return loadingFullRef.current.has(sessionId)
  }, [])

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

  useEffect(() => {
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
  }, [])

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
    getSessionFull,
    loadSessionFull,
    isLoadingFull,
    pagination,
    loadMore,
    loadingMore,
    connectionStatus,
    version,
  }
}
