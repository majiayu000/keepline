import { useEffect, useCallback } from 'react'

interface KeyboardShortcuts {
  onRefresh?: () => void
  onSync?: () => void
}

export function useKeyboardShortcuts({ onRefresh, onSync }: KeyboardShortcuts) {
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    // Ignore if typing in an input
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
      return
    }

    // Cmd/Ctrl + R: Refresh
    if ((e.metaKey || e.ctrlKey) && e.key === 'r') {
      e.preventDefault()
      onRefresh?.()
    }

    // Cmd/Ctrl + S: Sync
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault()
      onSync?.()
    }

    // Just 'r' without modifier: Refresh
    if (e.key === 'r' && !e.metaKey && !e.ctrlKey && !e.altKey) {
      onRefresh?.()
    }

    // Just 's' without modifier: Sync
    if (e.key === 's' && !e.metaKey && !e.ctrlKey && !e.altKey) {
      onSync?.()
    }
  }, [onRefresh, onSync])

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('keydown', handleKeyDown)
    }
  }, [handleKeyDown])
}
