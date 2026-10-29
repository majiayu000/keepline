import { useEffect, useCallback } from 'react'

interface KeyboardShortcuts {
  onRefresh?: () => void
  onSync?: () => void
  onShowHelp?: () => void
}

export function useKeyboardShortcuts({
  onRefresh,
  onSync,
  onShowHelp,
}: KeyboardShortcuts) {
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    // Ignore if typing in an input
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
      return
    }

    // Ignore if modifier keys are pressed (except for specific combos)
    const hasModifier = e.metaKey || e.ctrlKey || e.altKey

    // Cmd/Ctrl + R: Refresh (prevent browser refresh)
    if ((e.metaKey || e.ctrlKey) && e.key === 'r') {
      e.preventDefault()
      onRefresh?.()
      return
    }

    // Cmd/Ctrl + S: Sync (prevent browser save)
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault()
      onSync?.()
      return
    }

    // Skip other shortcuts if modifier is pressed
    if (hasModifier) return

    // Single key shortcuts
    switch (e.key) {
      case 'r':
        onRefresh?.()
        break
      case 's':
        onSync?.()
        break
      case '?':
        e.preventDefault()
        onShowHelp?.()
        break
    }
  }, [onRefresh, onSync, onShowHelp])

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('keydown', handleKeyDown)
    }
  }, [handleKeyDown])
}
