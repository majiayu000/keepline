import { useCallback, useEffect, useRef, useState } from 'react'
import { api } from '@/services/api'
import type { OrchestratorOverviewData } from '@/types'

export interface UseOrchestratorOverviewReturn {
  overview: OrchestratorOverviewData | null
  loading: boolean
  error: string | null
  refresh: () => Promise<void>
}

export function useOrchestratorOverview(_token: string): UseOrchestratorOverviewReturn {
  const [overview, setOverview] = useState<OrchestratorOverviewData | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const mountedRef = useRef(true)
  const refreshRequestIdRef = useRef(0)

  const refresh = useCallback(async () => {
    const requestId = ++refreshRequestIdRef.current
    const response = await api.fetchOrchestratorOverview()
    if (!mountedRef.current || requestId !== refreshRequestIdRef.current) return

    if (response.success && response.data) {
      setOverview(response.data)
      setError(null)
    } else {
      setError(response.error || 'Failed to load orchestrator overview')
    }
    setLoading(false)
  }, [])

  useEffect(() => {
    mountedRef.current = true
    setLoading(true)
    refresh()
    return () => {
      mountedRef.current = false
    }
  }, [refresh])

  return {
    overview,
    loading,
    error,
    refresh,
  }
}
