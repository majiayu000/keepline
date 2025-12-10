import { useState, useCallback, useRef, useEffect } from 'react'

interface AsyncState<T> {
  data: T | null
  loading: boolean
  error: string | null
}

interface UseAsyncReturn<T> extends AsyncState<T> {
  execute: () => Promise<T | null>
  reset: () => void
}

/**
 * Generic hook for handling async operations with loading/error states
 */
export function useAsync<T>(
  asyncFunction: (signal?: AbortSignal) => Promise<{ success: boolean; data?: T; error?: string }>,
  immediate = false
): UseAsyncReturn<T> {
  const [state, setState] = useState<AsyncState<T>>({
    data: null,
    loading: immediate,
    error: null,
  })

  const abortControllerRef = useRef<AbortController | null>(null)

  const execute = useCallback(async (): Promise<T | null> => {
    // Cancel any in-flight request
    if (abortControllerRef.current) {
      abortControllerRef.current.abort()
    }

    abortControllerRef.current = new AbortController()

    setState(prev => ({ ...prev, loading: true, error: null }))

    try {
      const response = await asyncFunction(abortControllerRef.current.signal)

      if (response.success && response.data) {
        setState({ data: response.data, loading: false, error: null })
        return response.data
      } else {
        setState({ data: null, loading: false, error: response.error || 'Unknown error' })
        return null
      }
    } catch (err) {
      // Don't update state if request was aborted
      if (err instanceof Error && err.name === 'AbortError') {
        return null
      }
      const errorMessage = err instanceof Error ? err.message : 'Unknown error'
      setState({ data: null, loading: false, error: errorMessage })
      return null
    }
  }, [asyncFunction])

  const reset = useCallback(() => {
    if (abortControllerRef.current) {
      abortControllerRef.current.abort()
    }
    setState({ data: null, loading: false, error: null })
  }, [])

  // Execute immediately if requested
  useEffect(() => {
    if (immediate) {
      execute()
    }

    return () => {
      if (abortControllerRef.current) {
        abortControllerRef.current.abort()
      }
    }
  }, [immediate, execute])

  return {
    ...state,
    execute,
    reset,
  }
}
