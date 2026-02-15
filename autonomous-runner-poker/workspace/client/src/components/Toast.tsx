import { useEffect, useState } from 'react'
import { useToast, type Toast as ToastType, type ToastType as ToastVariant } from '../contexts/ToastContext'

// Toast icons for each type
const TOAST_ICONS: Record<ToastVariant, string> = {
  success: '✓',
  error: '✕',
  warning: '⚠',
  info: 'ℹ',
}

// Toast colors for each type
const TOAST_COLORS: Record<ToastVariant, { bg: string; border: string; text: string; icon: string }> = {
  success: {
    bg: 'bg-green-900/90',
    border: 'border-green-500',
    text: 'text-green-100',
    icon: 'text-green-400',
  },
  error: {
    bg: 'bg-red-900/90',
    border: 'border-red-500',
    text: 'text-red-100',
    icon: 'text-red-400',
  },
  warning: {
    bg: 'bg-yellow-900/90',
    border: 'border-yellow-500',
    text: 'text-yellow-100',
    icon: 'text-yellow-400',
  },
  info: {
    bg: 'bg-blue-900/90',
    border: 'border-blue-500',
    text: 'text-blue-100',
    icon: 'text-blue-400',
  },
}

interface ToastItemProps {
  toast: ToastType
  onDismiss: () => void
}

function ToastItem({ toast, onDismiss }: ToastItemProps) {
  const [isVisible, setIsVisible] = useState(false)
  const [isExiting, setIsExiting] = useState(false)
  const colors = TOAST_COLORS[toast.type]
  const icon = TOAST_ICONS[toast.type]

  // Animate in on mount
  useEffect(() => {
    const timer = setTimeout(() => setIsVisible(true), 10)
    return () => clearTimeout(timer)
  }, [])

  // Handle exit animation
  const handleDismiss = () => {
    setIsExiting(true)
    setTimeout(onDismiss, 200) // Wait for exit animation
  }

  // Progress bar width calculation
  const [progress, setProgress] = useState(100)
  useEffect(() => {
    if (toast.duration <= 0) return

    const startTime = toast.createdAt
    const endTime = startTime + toast.duration

    const updateProgress = () => {
      const now = Date.now()
      const remaining = endTime - now
      const newProgress = Math.max(0, (remaining / toast.duration) * 100)
      setProgress(newProgress)

      if (newProgress > 0) {
        requestAnimationFrame(updateProgress)
      }
    }

    const animationId = requestAnimationFrame(updateProgress)
    return () => cancelAnimationFrame(animationId)
  }, [toast.createdAt, toast.duration])

  return (
    <div
      className={`
        relative overflow-hidden rounded-lg shadow-lg backdrop-blur-sm
        border-l-4 ${colors.border} ${colors.bg}
        transform transition-all duration-200 ease-out
        ${isVisible && !isExiting
          ? 'translate-x-0 opacity-100'
          : 'translate-x-full opacity-0'
        }
        min-w-[280px] max-w-[380px]
      `}
      role="alert"
    >
      <div className="flex items-start gap-3 p-3">
        {/* Icon */}
        <span className={`flex-shrink-0 text-lg font-bold ${colors.icon}`}>
          {icon}
        </span>

        {/* Message */}
        <p className={`flex-1 text-sm ${colors.text}`}>
          {toast.message}
        </p>

        {/* Dismiss button */}
        <button
          onClick={handleDismiss}
          className={`flex-shrink-0 ${colors.text} hover:opacity-70 transition-opacity p-0.5`}
          aria-label="Dismiss"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      {/* Progress bar */}
      {toast.duration > 0 && (
        <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-black/20">
          <div
            className={`h-full ${colors.border.replace('border-', 'bg-')} transition-none`}
            style={{ width: `${progress}%` }}
          />
        </div>
      )}
    </div>
  )
}

// Toast container that displays all toasts
export default function ToastContainer() {
  const { toasts, removeToast } = useToast()

  if (toasts.length === 0) return null

  return (
    <div
      className="fixed top-4 right-4 z-50 flex flex-col gap-2 pointer-events-none"
      aria-live="polite"
      aria-atomic="true"
    >
      {toasts.map((toast) => (
        <div key={toast.id} className="pointer-events-auto">
          <ToastItem
            toast={toast}
            onDismiss={() => removeToast(toast.id)}
          />
        </div>
      ))}
    </div>
  )
}

// Named export for using toast in specific locations
export { ToastItem }
