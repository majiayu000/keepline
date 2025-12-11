import { memo } from 'react'
import { ThemeSwitcher } from '@/components/ThemeSwitcher'
import { ExportMenu } from '@/components/ExportMenu'
import { Button } from '@/components/Button'
import type { Session } from '@/types'
import styles from './Header.module.css'

interface HeaderProps {
  onSync: () => void
  syncing?: boolean
  sessions?: Session[]
}

export const Header = memo(function Header({ onSync, syncing = false, sessions = [] }: HeaderProps) {
  return (
    <header className={styles.header} role="banner">
      <div className={styles.brand}>
        <h1 className={styles.title}>TASKER</h1>
        <span className={styles.subtitle}>Claude Code Monitor</span>
      </div>
      <div className={styles.actions}>
        <ExportMenu sessions={sessions} />
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
