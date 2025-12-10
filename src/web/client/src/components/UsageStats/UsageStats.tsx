import type { UsageStats as UsageStatsType } from '@/types'
import styles from './UsageStats.module.css'

interface UsageStatsProps {
  stats: UsageStatsType
  compact?: boolean
}

export function UsageStats({ stats, compact = false }: UsageStatsProps) {
  const formatTokens = (n: number): string => {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
    return n.toString()
  }

  const formatCost = (cost: number): string => {
    if (cost < 0.01) return `$${cost.toFixed(4)}`
    if (cost < 1) return `$${cost.toFixed(3)}`
    return `$${cost.toFixed(2)}`
  }

  if (compact) {
    return (
      <div className={styles.compact}>
        <span className={styles.tokens}>{formatTokens(stats.totalTokens)} tokens</span>
        <span className={styles.cost}>{formatCost(stats.totalCost)}</span>
      </div>
    )
  }

  return (
    <div className={styles.container}>
      <div className={styles.row}>
        <div className={styles.stat}>
          <span className={styles.label}>Input</span>
          <span className={styles.value}>{formatTokens(stats.totalInputTokens)}</span>
        </div>
        <div className={styles.stat}>
          <span className={styles.label}>Output</span>
          <span className={styles.value}>{formatTokens(stats.totalOutputTokens)}</span>
        </div>
        <div className={styles.stat}>
          <span className={styles.label}>Total</span>
          <span className={styles.value}>{formatTokens(stats.totalTokens)}</span>
        </div>
      </div>
      <div className={styles.row}>
        <div className={styles.stat}>
          <span className={styles.label}>API Calls</span>
          <span className={styles.value}>{stats.apiCalls}</span>
        </div>
        <div className={styles.stat}>
          <span className={styles.label}>Cost</span>
          <span className={styles.costValue}>{formatCost(stats.totalCost)}</span>
        </div>
      </div>
    </div>
  )
}
