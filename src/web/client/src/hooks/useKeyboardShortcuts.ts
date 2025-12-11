import { useEffect, useCallback } from 'react'
import type { Theme } from '@/contexts/ThemeContext'

interface KeyboardShortcuts {
  onRefresh?: () => void
  onSync?: () => void
  onShowHelp?: () => void
  onSetTheme?: (theme: Theme) => void
  onCycleTheme?: () => void
}

const THEME_KEYS: Record<string, Theme> = {
  '1': 'cyberpunk',
  '2': 'matrix',
  '3': 'synthwave',
  '4': 'minimal',
  '5': 'tokyo',
}

export function useKeyboardShortcuts({
  onRefresh,
  onSync,
  onShowHelp,
  onSetTheme,
  onCycleTheme,
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
      case 't':
        onCycleTheme?.()
        break
      case '1':
      case '2':
      case '3':
      case '4':
      case '5':
        if (THEME_KEYS[e.key]) {
          onSetTheme?.(THEME_KEYS[e.key])
        }
        break
    }
  }, [onRefresh, onSync, onShowHelp, onSetTheme, onCycleTheme])

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('keydown', handleKeyDown)
    }
  }, [handleKeyDown])
}
