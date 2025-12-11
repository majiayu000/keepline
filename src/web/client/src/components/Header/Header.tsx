import { memo } from 'react'
import { ThemeSwitcher } from '@/components/ThemeSwitcher'
import { ExportMenu } from '@/components/ExportMenu'
import { NotificationSettings } from '@/components/NotificationSettings'
import { Button } from '@/components/Button'
import type { Session } from '@/types'
import type { NotificationSettings as NotificationSettingsType } from '@/hooks'
import styles from './Header.module.css'

interface HeaderProps {
  onSync: () => void
  syncing?: boolean
  sessions?: Session[]
  // Notification props
  notificationSettings?: NotificationSettingsType
  onUpdateNotificationSettings?: (updates: Partial<NotificationSettingsType>) => void
  notificationPermission?: NotificationPermission
  onRequestNotificationPermission?: () => Promise<boolean>
}

export const Header = memo(function Header({
  onSync,
  syncing = false,
  sessions = [],
  notificationSettings,
  onUpdateNotificationSettings,
  notificationPermission,
  onRequestNotificationPermission,
}: HeaderProps) {
  return (
    <header className={styles.header} role="banner">
      <div className={styles.brand}>
        <h1 className={styles.title}>TASKER</h1>
        <span className={styles.subtitle}>Claude Code Monitor</span>
      </div>
      <div className={styles.actions}>
        <ExportMenu sessions={sessions} />
        {notificationSettings && onUpdateNotificationSettings && onRequestNotificationPermission && (
          <NotificationSettings
            settings={notificationSettings}
            onUpdateSettings={onUpdateNotificationSettings}
            permission={notificationPermission || 'default'}
            onRequestPermission={onRequestNotificationPermission}
          />
        )}
        <Button
          variant="secondary"
          size="sm"
          onClick={onSync}
          loading={syncing}
        >
          Sync
        </Button>
        <ThemeSwitcher />
      </div>
    </header>
  )
})
