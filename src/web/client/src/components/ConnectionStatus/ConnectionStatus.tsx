import { memo } from 'react'
import type { ConnectionStatus as ConnectionStatusType } from '@/hooks/useSessions'
import styles from './ConnectionStatus.module.css'

interface ConnectionStatusProps {
  status: ConnectionStatusType
}

const STATUS_LABELS: Record<ConnectionStatusType, string> = {
  realtime: 'Live',
  polling: 'Polling',
  disconnected: 'Offline',
}

export const ConnectionStatus = memo(function ConnectionStatus({ status }: ConnectionStatusProps) {
  return (
    <div className={styles.container} title={`Connection: ${status}`}>
      <span className={`${styles.dot} ${styles[status]}`} />
      <span className={`${styles.label} ${styles[status]}`}>
        {STATUS_LABELS[status]}
      </span>
    </div>
  )
})
