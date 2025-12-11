import { memo, useState, useRef, useCallback } from 'react'
import { useClickOutside } from '@/hooks'
import { exportToJSON, exportToCSV, exportToMarkdown } from '@/utils/export'
import type { Session } from '@/types'
import styles from './ExportMenu.module.css'

interface ExportMenuProps {
  sessions: Session[]
  disabled?: boolean
}

export const ExportMenu = memo(function ExportMenu({ sessions, disabled }: ExportMenuProps) {
  const [isOpen, setIsOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)

  useClickOutside(containerRef, () => setIsOpen(false))

  const handleExportJSON = useCallback(() => {
    const timestamp = new Date().toISOString().split('T')[0]
    exportToJSON(sessions, `claude-sessions-${timestamp}`)
    setIsOpen(false)
  }, [sessions])

  const handleExportCSV = useCallback(() => {
    const timestamp = new Date().toISOString().split('T')[0]
    exportToCSV(sessions, `claude-sessions-${timestamp}`)
    setIsOpen(false)
  }, [sessions])

  const handleExportMarkdown = useCallback(() => {
    const timestamp = new Date().toISOString().split('T')[0]
    exportToMarkdown(sessions, `claude-sessions-${timestamp}`)
    setIsOpen(false)
  }, [sessions])

  const isEmpty = sessions.length === 0

  return (
    <div className={styles.container} ref={containerRef}>
      <button
        className={styles.trigger}
        onClick={() => setIsOpen(!isOpen)}
        disabled={disabled || isEmpty}
        title={isEmpty ? 'No sessions to export' : 'Export sessions'}
      >
        <span className={styles.triggerIcon}>📥</span>
        Export
        <span className={`${styles.arrow} ${isOpen ? styles.open : ''}`}>▼</span>
      </button>

      {isOpen && (
        <div className={styles.menu}>
          <button className={styles.menuItem} onClick={handleExportJSON}>
            <span className={styles.menuIcon}>{ }</span>
            <span className={styles.menuLabel}>JSON</span>
            <span className={styles.menuHint}>Full data</span>
          </button>
          <button className={styles.menuItem} onClick={handleExportCSV}>
            <span className={styles.menuIcon}>📊</span>
            <span className={styles.menuLabel}>CSV</span>
            <span className={styles.menuHint}>Spreadsheet</span>
          </button>
          <div className={styles.divider} />
          <button className={styles.menuItem} onClick={handleExportMarkdown}>
            <span className={styles.menuIcon}>📝</span>
            <span className={styles.menuLabel}>Markdown</span>
            <span className={styles.menuHint}>Report</span>
          </button>
        </div>
      )}
    </div>
  )
})
