/**
 * useAuth - Authentication state management for web terminal
 */

import { useState, useCallback, useEffect } from 'react'
import { fetchAuthStatus, setupAuth, loginAuth, logoutAuth } from '@/services/api'
import type { AuthStatus } from '@/types'

const TOKEN_KEY = 'terminal_token'

export function useAuth() {
  const [status, setStatus] = useState<AuthStatus | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const checkStatus = useCallback(async () => {
    setLoading(true)
    setError(null)
    const res = await fetchAuthStatus()
    if (res.success && res.data) {
      setStatus(res.data)
    } else {
      setError(res.error || 'Failed to check auth status')
    }
    setLoading(false)
  }, [])

  useEffect(() => {
    checkStatus()
  }, [checkStatus])

  const setup = useCallback(async (username: string, password: string, enableTotp?: boolean) => {
    setError(null)
    const res = await setupAuth({ username, password, enableTotp })
    if (res.success && res.data) {
      localStorage.setItem(TOKEN_KEY, res.data.token)
      await checkStatus()
      return res.data
    }
    const msg = res.error || 'Setup failed'
    setError(msg)
    throw new Error(msg)
  }, [checkStatus])

  const login = useCallback(async (username: string, password: string, totpCode?: string) => {
    setError(null)
    const res = await loginAuth({ username, password, totpCode })
    if (res.success && res.data) {
      localStorage.setItem(TOKEN_KEY, res.data.token)
      await checkStatus()
      return
    }
    const msg = res.error || 'Login failed'
    setError(msg)
    throw new Error(msg)
  }, [checkStatus])

  const logout = useCallback(async () => {
    await logoutAuth()
    localStorage.removeItem(TOKEN_KEY)
    setStatus(prev => prev ? { ...prev, authenticated: false, username: undefined } : null)
  }, [])

  const getToken = useCallback(() => localStorage.getItem(TOKEN_KEY), [])

  return {
    status,
    loading,
    error,
    setup,
    login,
    logout,
    getToken,
    checkStatus,
  }
}
