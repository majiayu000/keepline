import { useState, useEffect, useCallback } from 'react'
import type { Session } from '@/types'

export interface NotificationSettings {
  enabled: boolean
  onStatusChange: boolean
  onSessionLost: boolean
  onHighCost: boolean
  costThreshold: number
}

const DEFAULT_SETTINGS: NotificationSettings = {
  enabled: true,
  onStatusChange: true,
  onSessionLost: true,
  onHighCost: true,
  costThreshold: 1.0, // $1.00
}

const STORAGE_KEY = 'tasker-notification-settings'

interface UseNotificationsReturn {
  settings: NotificationSettings
  updateSettings: (updates: Partial<NotificationSettings>) => void
  permission: NotificationPermission
  requestPermission: () => Promise<boolean>
  notify: (title: string, options?: NotificationOptions) => void
  checkSessionChanges: (prevSessions: Session[], newSessions: Session[]) => void
}

export function useNotifications(): UseNotificationsReturn {
  const [settings, setSettings] = useState<NotificationSettings>(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY)
      return saved ? { ...DEFAULT_SETTINGS, ...JSON.parse(saved) } : DEFAULT_SETTINGS
    } catch {
      return DEFAULT_SETTINGS
    }
  })

  const [permission, setPermission] = useState<NotificationPermission>(
    typeof Notification !== 'undefined' ? Notification.permission : 'denied'
  )

  // Save settings to localStorage
  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
  }, [settings])

  const updateSettings = useCallback((updates: Partial<NotificationSettings>) => {
    setSettings((prev) => ({ ...prev, ...updates }))
  }, [])

  const requestPermission = useCallback(async (): Promise<boolean> => {
    if (typeof Notification === 'undefined') {
      return false
    }

    if (Notification.permission === 'granted') {
      setPermission('granted')
      return true
    }

    if (Notification.permission === 'denied') {
      setPermission('denied')
      return false
    }

    try {
      const result = await Notification.requestPermission()
      setPermission(result)
      return result === 'granted'
    } catch {
      return false
    }
  }, [])

  const notify = useCallback((title: string, options?: NotificationOptions) => {
    if (!settings.enabled || permission !== 'granted') {
      return
    }

    try {
      const notification = new Notification(title, {
        icon: '/favicon.ico',
        badge: '/favicon.ico',
        tag: 'tasker-notification',
        ...options,
      })

      // Auto close after 5 seconds
      setTimeout(() => notification.close(), 5000)

      // Focus window on click
      notification.onclick = () => {
        window.focus()
        notification.close()
      }
    } catch (error) {
      console.error('Failed to show notification:', error)
    }
  }, [settings.enabled, permission])

  const checkSessionChanges = useCallback((prevSessions: Session[], newSessions: Session[]) => {
    if (!settings.enabled || permission !== 'granted') {
      return
    }

    const prevMap = new Map(prevSessions.map((s) => [s.sessionId, s]))

    for (const session of newSessions) {
      const prev = prevMap.get(session.sessionId)

      // New session
      if (!prev) {
        continue // Don't notify for new sessions
      }

      // Status changed
      if (settings.onStatusChange && prev.status !== session.status) {
        // Session became lost
        if (settings.onSessionLost && session.status === 'lost') {
          notify('Session Lost', {
            body: `Session "${session.title || session.sessionId}" has been lost`,
            tag: `session-lost-${session.sessionId}`,
          })
        }
        // Session completed
        else if (session.status === 'completed' && prev.status !== 'completed') {
          notify('Session Completed', {
            body: `Session "${session.title || session.sessionId}" has completed`,
            tag: `session-completed-${session.sessionId}`,
          })
        }
      }

      // High cost warning
      if (settings.onHighCost && session.usageStats) {
        const prevCost = prev.usageStats?.totalCost || 0
        const newCost = session.usageStats.totalCost || 0

        // Check if cost crossed the threshold
        if (prevCost < settings.costThreshold && newCost >= settings.costThreshold) {
          notify('High Cost Warning', {
            body: `Session "${session.title || session.sessionId}" has exceeded $${settings.costThreshold.toFixed(2)} (current: $${newCost.toFixed(2)})`,
            tag: `high-cost-${session.sessionId}`,
          })
        }
      }
    }
  }, [settings, permission, notify])

  return {
    settings,
    updateSettings,
    permission,
    requestPermission,
    notify,
    checkSessionChanges,
  }
}
