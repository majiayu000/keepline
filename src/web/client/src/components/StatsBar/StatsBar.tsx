import { memo } from 'react'
import type { SessionStats } from '@/types'
import styles from './StatsBar.module.css'

interface StatsBarProps {
  stats: SessionStats | null
  loading?: boolean
}

export const StatsBar = memo(function StatsBar({ stats, loading }: StatsBarProps) {
  if (loading || !stats) {
    return (
      <div className={styles.container}>
        <div className={styles.placeholder}>Loading stats...</div>
      </div>
    )
  }

  return (
    <div className={styles.container}>
      {!!stats.needs_input && <StatItem label="Needs input" value={stats.needs_input} variant="warning" />}
      {!!stats.stalled && <StatItem label="Stalled" value={stats.stalled} variant="danger" />}
      <StatItem label="Running" value={stats.running} variant="success" />
      <StatItem label="Waiting" value={stats.waiting} variant="warning" />
      <StatItem label="Idle" value={stats.idle} variant="info" />
      <StatItem label="Interrupted" value={stats.lost + (stats.interrupted ?? 0)} variant="danger" />
      <div className={styles.divider} />
      <StatItem label="Total" value={stats.total} variant="default" />
    </div>
  )
})

type StatVariant = 'success' | 'warning' | 'info' | 'danger' | 'default'

interface StatItemProps {
  label: string
  value: number
  variant: StatVariant
}

const StatItem = memo(function StatItem({ label, value, variant }: StatItemProps) {
  return (
    <div className={styles.stat}>
      <span className={`${styles.value} ${styles[variant]}`}>{value}</span>
      <span className={styles.label}>{label}</span>
    </div>
  )
})
