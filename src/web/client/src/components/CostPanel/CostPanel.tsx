import { memo, useMemo, useState } from 'react'
import type { Session } from '@/types'
import { formatTokens, formatCost } from '@/utils/format'
import styles from './CostPanel.module.css'

interface CostPanelProps {
  sessions: Session[]
}

interface CostSummary {
  totalCost: number
  totalInputTokens: number
  totalOutputTokens: number
  totalTokens: number
  totalApiCalls: number
  sessionCount: number
  avgCostPerSession: number
  byStatus: Record<string, { count: number; cost: number; tokens: number }>
}

const STATUS_COLORS: Record<string, string> = {
  running: 'var(--success)',
  waiting: 'var(--warning)',
  idle: 'var(--info)',
  lost: 'var(--danger)',
  completed: 'var(--text-dim)',
}

export const CostPanel = memo(function CostPanel({ sessions }: CostPanelProps) {
  const [collapsed, setCollapsed] = useState(false)

  const summary = useMemo((): CostSummary => {
    const byStatus: Record<string, { count: number; cost: number; tokens: number }> = {}
    let totalCost = 0
    let totalInputTokens = 0
    let totalOutputTokens = 0
    let totalApiCalls = 0

    for (const session of sessions) {
      const stats = session.usageStats
      if (stats) {
        totalCost += stats.totalCost || 0
        totalInputTokens += stats.totalInputTokens || 0
        totalOutputTokens += stats.totalOutputTokens || 0
        totalApiCalls += stats.apiCalls || 0
      }

      const status = session.status
      if (!byStatus[status]) {
        byStatus[status] = { count: 0, cost: 0, tokens: 0 }
      }
      byStatus[status].count++
      byStatus[status].cost += stats?.totalCost || 0
      byStatus[status].tokens += stats?.totalTokens || 0
    }

    return {
      totalCost,
      totalInputTokens,
      totalOutputTokens,
      totalTokens: totalInputTokens + totalOutputTokens,
      totalApiCalls,
      sessionCount: sessions.length,
      avgCostPerSession: sessions.length > 0 ? totalCost / sessions.length : 0,
      byStatus,
    }
  }, [sessions])

  if (sessions.length === 0) {
    return null
  }

  return (
    <div className={styles.panel}>
      <div className={styles.header} onClick={() => setCollapsed(!collapsed)}>
        <div className={styles.headerLeft}>
          <span className={styles.icon}>💰</span>
          <h3 className={styles.title}>Cost Analysis</h3>
        </div>
        <div className={styles.headerLeft}>
          <span className={styles.totalCost}>{formatCost(summary.totalCost)}</span>
          <span className={`${styles.collapseIcon} ${collapsed ? styles.collapsed : ''}`}>▼</span>
        </div>
      </div>

      {!collapsed && (
        <div className={styles.content}>
          {/* Summary Grid */}
          <div className={styles.grid}>
            <div className={styles.statCard}>
              <span className={styles.statLabel}>Total Cost</span>
              <span className={`${styles.statValue} ${styles.highlight}`}>
                {formatCost(summary.totalCost)}
              </span>
            </div>
            <div className={styles.statCard}>
              <span className={styles.statLabel}>Sessions</span>
              <span className={styles.statValue}>{summary.sessionCount}</span>
              <span className={styles.statSubtext}>
                Avg: {formatCost(summary.avgCostPerSession)}/session
              </span>
            </div>
            <div className={styles.statCard}>
              <span className={styles.statLabel}>Input Tokens</span>
              <span className={styles.statValue}>{formatTokens(summary.totalInputTokens)}</span>
            </div>
            <div className={styles.statCard}>
              <span className={styles.statLabel}>Output Tokens</span>
              <span className={styles.statValue}>{formatTokens(summary.totalOutputTokens)}</span>
            </div>
            <div className={styles.statCard}>
              <span className={styles.statLabel}>Total Tokens</span>
              <span className={styles.statValue}>{formatTokens(summary.totalTokens)}</span>
            </div>
            <div className={styles.statCard}>
              <span className={styles.statLabel}>API Calls</span>
              <span className={styles.statValue}>{summary.totalApiCalls.toLocaleString()}</span>
            </div>
          </div>

          {/* Cost by Status */}
          <div className={styles.section}>
            <h4 className={styles.sectionTitle}>Cost by Status</h4>
            <div className={styles.breakdown}>
              {Object.entries(summary.byStatus)
                .sort((a, b) => b[1].cost - a[1].cost)
                .map(([status, data]) => (
                  <div key={status} className={styles.breakdownItem}>
                    <div className={styles.breakdownLabel}>
                      <span
                        className={styles.breakdownDot}
                        style={{ backgroundColor: STATUS_COLORS[status] || 'var(--text-dim)' }}
                      />
                      <span>
                        {status.charAt(0).toUpperCase() + status.slice(1)} ({data.count})
                      </span>
                    </div>
                    <span className={styles.breakdownValue}>{formatCost(data.cost)}</span>
                  </div>
                ))}
            </div>
          </div>

          {/* Token Distribution */}
          <div className={styles.section}>
            <h4 className={styles.sectionTitle}>Token Distribution</h4>
            <div className={styles.breakdown}>
              <div className={styles.breakdownItem}>
                <div className={styles.breakdownLabel}>
                  <span
                    className={styles.breakdownDot}
                    style={{ backgroundColor: 'var(--info)' }}
                  />
                  <span>Input</span>
                </div>
                <span className={styles.breakdownValue}>
                  {summary.totalTokens > 0
                    ? ((summary.totalInputTokens / summary.totalTokens) * 100).toFixed(1)
                    : 0}%
                </span>
              </div>
              <div className={styles.progressBar}>
                <div
                  className={styles.progressFill}
                  style={{
                    width: summary.totalTokens > 0
                      ? `${(summary.totalInputTokens / summary.totalTokens) * 100}%`
                      : '0%',
                    backgroundColor: 'var(--info)',
                  }}
                />
              </div>
              <div className={styles.breakdownItem}>
                <div className={styles.breakdownLabel}>
                  <span
                    className={styles.breakdownDot}
                    style={{ backgroundColor: 'var(--success)' }}
                  />
                  <span>Output</span>
                </div>
                <span className={styles.breakdownValue}>
                  {summary.totalTokens > 0
                    ? ((summary.totalOutputTokens / summary.totalTokens) * 100).toFixed(1)
                    : 0}%
                </span>
              </div>
              <div className={styles.progressBar}>
                <div
                  className={styles.progressFill}
                  style={{
                    width: summary.totalTokens > 0
                      ? `${(summary.totalOutputTokens / summary.totalTokens) * 100}%`
                      : '0%',
                    backgroundColor: 'var(--success)',
                  }}
                />
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
})
