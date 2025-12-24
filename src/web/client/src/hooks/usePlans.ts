import { useState, useEffect, useCallback, useRef } from 'react'
import { api } from '@/services/api'
import type { Plan, PlanAggregateStats } from '@/types'

interface UsePlansReturn {
  plans: Plan[]
  stats: PlanAggregateStats | null
  loading: boolean
  error: string | null
  refresh: () => Promise<void>
  getPlan: (id: string) => Plan | undefined
  loadPlan: (id: string) => Promise<Plan | null>
  isLoadingPlan: (id: string) => boolean
}

export function usePlans(): UsePlansReturn {
  const [plans, setPlans] = useState<Plan[]>([])
  const [stats, setStats] = useState<PlanAggregateStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const mountedRef = useRef(true)

  // Cache for full plan content
  const planCacheRef = useRef<Map<string, Plan>>(new Map())
  const loadingPlanRef = useRef<Set<string>>(new Set())
  const [, forceUpdate] = useState(0)

  // Fetch all plans and stats
  const refresh = useCallback(async () => {
    try {
      setLoading(true)
      setError(null)

      const [plansRes, statsRes] = await Promise.all([
        api.fetchPlans(),
        api.fetchPlanStats(),
      ])

      if (!mountedRef.current) return

      if (plansRes.success && plansRes.data) {
        setPlans(plansRes.data)
        // Update cache with basic plan data
        plansRes.data.forEach(p => {
          if (!planCacheRef.current.has(p.id)) {
            planCacheRef.current.set(p.id, p)
          }
        })
      }

      if (statsRes.success && statsRes.data) {
        setStats(statsRes.data)
      }

      if (!plansRes.success) {
        setError(plansRes.error || 'Failed to fetch plans')
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

  // Get cached plan
  const getPlan = useCallback((id: string): Plan | undefined => {
    return planCacheRef.current.get(id)
  }, [])

  // Load full plan with content
  const loadPlan = useCallback(async (id: string): Promise<Plan | null> => {
    // Check cache - only return if has content
    const cached = planCacheRef.current.get(id)
    if (cached?.content) return cached

    // Check if already loading
    if (loadingPlanRef.current.has(id)) return null

    loadingPlanRef.current.add(id)
    forceUpdate(n => n + 1)

    try {
      const res = await api.fetchPlan(id)
      if (res.success && res.data) {
        planCacheRef.current.set(id, res.data)
        forceUpdate(n => n + 1)
        return res.data
      }
      return null
    } finally {
      loadingPlanRef.current.delete(id)
      forceUpdate(n => n + 1)
    }
  }, [])

  // Check if loading a specific plan
  const isLoadingPlan = useCallback((id: string): boolean => {
    return loadingPlanRef.current.has(id)
  }, [])

  return {
    plans,
    stats,
    loading,
    error,
    refresh,
    getPlan,
    loadPlan,
    isLoadingPlan,
  }
}
