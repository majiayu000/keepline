import { memo } from 'react'
import type { ConnectionStatus as ConnectionStatusType } from '@/hooks/useSessions'
import styles from './ConnectionStatus.module.css'

interface ConnectionStatusProps {
  status: ConnectionStatusType
}

const STATUS_LABELS: Record<ConnectionStatusType, string> = {
  realtime: '实时',
  polling: '定时刷新',
  disconnected: '离线',
}

export const ConnectionStatus = memo(function ConnectionStatus({ status }: ConnectionStatusProps) {
  return (
    <div className={styles.container} title={`连接状态：${STATUS_LABELS[status]}`}>
      <span className={`${styles.dot} ${styles[status]}`} />
      <span className={`${styles.label} ${styles[status]}`}>
        {STATUS_LABELS[status]}
      </span>
    </div>
  )
})
