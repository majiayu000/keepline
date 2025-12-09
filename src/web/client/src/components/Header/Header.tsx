import { ThemeSwitcher } from '@/components/ThemeSwitcher'
import { Button } from '@/components/Button'
import styles from './Header.module.css'

interface HeaderProps {
  onSync: () => void
  syncing?: boolean
}

export function Header({ onSync, syncing = false }: HeaderProps) {
  return (
    <header className={styles.header}>
      <div className={styles.brand}>
        <h1 className={styles.title}>TASKER</h1>
        <span className={styles.subtitle}>Claude Code Monitor</span>
      </div>
      <div className={styles.actions}>
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
}
