import { useState, useMemo, useCallback } from 'react'
import type { Session, SessionStatus } from '@/types'

interface UseSessionFilterReturn {
  searchQuery: string
  setSearchQuery: (query: string) => void
  statusFilters: Set<SessionStatus>
  setStatusFilters: (filters: Set<SessionStatus>) => void
  filteredSessions: Session[]
  clearFilters: () => void
  hasActiveFilters: boolean
}

export function useSessionFilter(sessions: Session[]): UseSessionFilterReturn {
  const [searchQuery, setSearchQuery] = useState('')
  const [statusFilters, setStatusFilters] = useState<Set<SessionStatus>>(new Set())

  const hasActiveFilters = searchQuery.length > 0 || statusFilters.size > 0

  const clearFilters = useCallback(() => {
    setSearchQuery('')
    setStatusFilters(new Set())
  }, [])

  const filteredSessions = useMemo(() => {
    let result = sessions

    // Apply status filter
    if (statusFilters.size > 0) {
      result = result.filter((session) => statusFilters.has(session.status as SessionStatus))
    }

    // Apply search filter
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase().trim()
      result = result.filter((session) => {
        // Search in title
        if (session.title?.toLowerCase().includes(query)) return true
        // Search in directory
        if (session.directory?.toLowerCase().includes(query)) return true
        // Search in initial prompt
        if (session.initialPrompt?.toLowerCase().includes(query)) return true
        // Search in session ID
        if (session.sessionId?.toLowerCase().includes(query)) return true
        // Search in last tool
        if (session.lastTool?.toLowerCase().includes(query)) return true
        // Search in current file
        if (session.currentFile?.toLowerCase().includes(query)) return true
        return false
      })
    }

    return result
  }, [sessions, searchQuery, statusFilters])

  return {
    searchQuery,
    setSearchQuery,
    statusFilters,
    setStatusFilters,
    filteredSessions,
    clearFilters,
    hasActiveFilters,
  }
}
