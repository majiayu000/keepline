import { useState, useEffect, useCallback, useRef } from 'react'
import { api } from '@/services/api'
import type { Session, SessionStats } from '@/types'

const REFRESH_INTERVAL = 30000 // 30 seconds

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
}

export function useSessions(): UseSessionsReturn {
  const [sessions, setSessions] = useState<Session[]>([])
  const [stats, setStats] = useState<SessionStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [syncing, setSyncing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const intervalRef = useRef<number | null>(null)

  const loadSessions = useCallback(async () => {
    const response = await api.fetchSessions()

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
    setLoading(false)
  }, [loadSessions])

  const sync = useCallback(async () => {
    setSyncing(true)
    const response = await api.syncSessions()
    const success = response.success
    if (success) {
      await loadSessions()
    }
    setSyncing(false)
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

  // Initial load
  useEffect(() => {
    refresh()
  }, [refresh])

  // Auto-refresh
  useEffect(() => {
    intervalRef.current = window.setInterval(() => {
      loadSessions()
    }, REFRESH_INTERVAL)

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
  }
}
