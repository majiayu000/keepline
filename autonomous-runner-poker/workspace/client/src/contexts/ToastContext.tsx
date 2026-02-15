import { createContext, useContext, useState, useCallback, type ReactNode } from 'react'

export type ToastType = 'success' | 'error' | 'warning' | 'info'

export interface Toast {
  id: string
  message: string
  type: ToastType
  duration: number
  createdAt: number
}

interface ToastContextValue {
  toasts: Toast[]
  addToast: (message: string, type?: ToastType, duration?: number) => string
  removeToast: (id: string) => void
  clearAllToasts: () => void
  // Convenience methods
  success: (message: string, duration?: number) => string
  error: (message: string, duration?: number) => string
  warning: (message: string, duration?: number) => string
  info: (message: string, duration?: number) => string
}

const ToastContext = createContext<ToastContextValue | null>(null)

// Default durations for different toast types (in ms)
const DEFAULT_DURATIONS: Record<ToastType, number> = {
  success: 3000,
  error: 5000,
  warning: 4000,
  info: 3000,
}

// Maximum number of toasts to show at once
const MAX_TOASTS = 5

let toastIdCounter = 0

function generateToastId(): string {
  return `toast-${Date.now()}-${++toastIdCounter}`
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([])

  const removeToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((toast) => toast.id !== id))
  }, [])

  const addToast = useCallback((
    message: string,
    type: ToastType = 'info',
    duration?: number
  ): string => {
    const id = generateToastId()
    const finalDuration = duration ?? DEFAULT_DURATIONS[type]

    const newToast: Toast = {
      id,
      message,
      type,
      duration: finalDuration,
      createdAt: Date.now(),
    }

    setToasts((prev) => {
      // Remove oldest toasts if we exceed the limit
      const updated = [...prev, newToast]
      if (updated.length > MAX_TOASTS) {
        return updated.slice(-MAX_TOASTS)
      }
      return updated
    })

    // Auto-remove after duration
    if (finalDuration > 0) {
      setTimeout(() => {
        removeToast(id)
      }, finalDuration)
    }

    return id
  }, [removeToast])

  const clearAllToasts = useCallback(() => {
    setToasts([])
  }, [])

  // Convenience methods
  const success = useCallback((message: string, duration?: number) => {
    return addToast(message, 'success', duration)
  }, [addToast])

  const error = useCallback((message: string, duration?: number) => {
    return addToast(message, 'error', duration)
  }, [addToast])

  const warning = useCallback((message: string, duration?: number) => {
    return addToast(message, 'warning', duration)
  }, [addToast])

  const info = useCallback((message: string, duration?: number) => {
    return addToast(message, 'info', duration)
  }, [addToast])

  const value: ToastContextValue = {
    toasts,
    addToast,
    removeToast,
    clearAllToasts,
    success,
    error,
    warning,
    info,
  }

  return (
    <ToastContext.Provider value={value}>
      {children}
    </ToastContext.Provider>
  )
}

export function useToast(): ToastContextValue {
  const context = useContext(ToastContext)
  if (!context) {
    throw new Error('useToast must be used within a ToastProvider')
  }
  return context
}
