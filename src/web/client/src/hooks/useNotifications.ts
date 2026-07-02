import { useState, useEffect, useCallback } from 'react'
import type { Session } from '@/types'
import { getSessionNotificationEvents } from './notification-events'

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

const STORAGE_KEY = 'keepline-notification-settings'

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
        tag: 'keepline-notification',
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

    for (const event of getSessionNotificationEvents(prevSessions, newSessions, settings)) {
      notify(event.title, event.options)
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
