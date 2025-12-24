import { useState, useEffect, useCallback, useRef } from 'react'
import { api } from '@/services/api'
import type { SessionMemory, MemorySummary, MemoryContext } from '@/types'

interface UseMemoryReturn {
  memories: SessionMemory[]
  summaries: MemorySummary[]
  loading: boolean
  error: string | null
  refresh: () => Promise<void>
  getMemory: (sessionId: string) => SessionMemory | undefined
  loadMemory: (sessionId: string) => Promise<SessionMemory | null>
  getContext: (sessionId: string) => MemoryContext | undefined
  loadContext: (sessionId: string, minimal?: boolean) => Promise<MemoryContext | null>
  deleteMemory: (sessionId: string) => Promise<boolean>
  isLoadingMemory: (sessionId: string) => boolean
}

export function useMemory(): UseMemoryReturn {
  const [memories, setMemories] = useState<SessionMemory[]>([])
  const [summaries, setSummaries] = useState<MemorySummary[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const mountedRef = useRef(true)

  // Cache for individual memory lookups
  const memoryCacheRef = useRef<Map<string, SessionMemory>>(new Map())
  const contextCacheRef = useRef<Map<string, MemoryContext>>(new Map())
  const loadingMemoryRef = useRef<Set<string>>(new Set())
  const [, forceUpdate] = useState(0)

  // Fetch all memories and summaries
  const refresh = useCallback(async () => {
    try {
      setLoading(true)
      setError(null)

      const [memoriesRes, summariesRes] = await Promise.all([
        api.fetchMemories({ limit: 100 }),
        api.fetchMemorySummaries(),
      ])

      if (!mountedRef.current) return

      if (memoriesRes.success && memoriesRes.data) {
        setMemories(memoriesRes.data)
        // Update cache
        memoriesRes.data.forEach(m => memoryCacheRef.current.set(m.sessionId, m))
      }

      if (summariesRes.success && summariesRes.data) {
        setSummaries(summariesRes.data)
      }

      if (!memoriesRes.success) {
        setError(memoriesRes.error || 'Failed to fetch memories')
      }
    } catch (err) {
      if (mountedRef.current) {
        setError(err instanceof Error ? err.message : 'Unknown error')
      }
    } finally {
      if (mountedRef.current) {
        setLoading(false)
      }
    }
  }, [])

  // Initial load
  useEffect(() => {
    mountedRef.current = true
    refresh()
    return () => {
      mountedRef.current = false
    }
  }, [refresh])

  // Get cached memory
  const getMemory = useCallback((sessionId: string): SessionMemory | undefined => {
    return memoryCacheRef.current.get(sessionId)
  }, [])

  // Load memory for a session
  const loadMemory = useCallback(async (sessionId: string): Promise<SessionMemory | null> => {
    // Check cache first
    const cached = memoryCacheRef.current.get(sessionId)
    if (cached) return cached

    // Check if already loading
    if (loadingMemoryRef.current.has(sessionId)) return null

    loadingMemoryRef.current.add(sessionId)
    forceUpdate(n => n + 1)

    try {
      const res = await api.fetchMemory(sessionId)
      if (res.success && res.data) {
        memoryCacheRef.current.set(sessionId, res.data)
        forceUpdate(n => n + 1)
        return res.data
      }
      return null
    } finally {
      loadingMemoryRef.current.delete(sessionId)
      forceUpdate(n => n + 1)
    }
  }, [])

  // Get cached context
  const getContext = useCallback((sessionId: string): MemoryContext | undefined => {
    return contextCacheRef.current.get(sessionId)
  }, [])

  // Load context for a session
  const loadContext = useCallback(async (sessionId: string, minimal?: boolean): Promise<MemoryContext | null> => {
    // Check cache first (only for non-minimal)
    if (!minimal) {
      const cached = contextCacheRef.current.get(sessionId)
      if (cached) return cached
    }

    try {
      const res = await api.fetchMemoryContext(sessionId, minimal)
      if (res.success && res.data) {
        if (!minimal) {
          contextCacheRef.current.set(sessionId, res.data)
        }
        return res.data
      }
      return null
    } catch {
      return null
    }
  }, [])

  // Delete memory
  const handleDeleteMemory = useCallback(async (sessionId: string): Promise<boolean> => {
    try {
      const res = await api.deleteMemory(sessionId)
      if (res.success) {
        // Remove from cache and state
        memoryCacheRef.current.delete(sessionId)
        contextCacheRef.current.delete(sessionId)
        setMemories(prev => prev.filter(m => m.sessionId !== sessionId))
        setSummaries(prev => prev.filter(s => s.sessionId !== sessionId))
        return true
      }
      return false
    } catch {
      return false
    }
  }, [])

  // Check if loading a specific memory
  const isLoadingMemory = useCallback((sessionId: string): boolean => {
    return loadingMemoryRef.current.has(sessionId)
  }, [])

  return {
    memories,
    summaries,
    loading,
    error,
    refresh,
    getMemory,
    loadMemory,
    getContext,
    loadContext,
    deleteMemory: handleDeleteMemory,
    isLoadingMemory,
  }
}
