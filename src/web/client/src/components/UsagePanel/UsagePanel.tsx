import { useState, useEffect, useCallback, memo } from 'react'
import type { DailyUsage, MonthlyUsage, QuotaData, QuotaWindow } from '@/types'
import { api } from '@/services/api'
import { Spinner } from '@/components/Spinner'
import { formatCost, formatTokens } from '@/utils/format'
import styles from './UsagePanel.module.css'

type ViewType = 'daily' | 'monthly'

/** Get color class based on utilization percentage */
function getUtilizationLevel(utilization: number): 'low' | 'medium' | 'high' {
  if (utilization >= 80) return 'high'
  if (utilization >= 50) return 'medium'
  return 'low'
}

/** Format reset time as relative string */
function formatResetTime(resetsAt: string | null): string {
  if (!resetsAt) return 'N/A'

  const resetDate = new Date(resetsAt)
  const now = new Date()
  const diffMs = resetDate.getTime() - now.getTime()

  if (diffMs <= 0) return 'Resetting...'

  const hours = Math.floor(diffMs / (1000 * 60 * 60))
  const minutes = Math.floor((diffMs % (1000 * 60 * 60)) / (1000 * 60))

  if (hours > 0) {
    return `Resets in ${hours}h ${minutes}m`
  }
  return `Resets in ${minutes}m`
}

/** Quota bar component */
function QuotaBar({ window, label }: { window: QuotaWindow; label: string }) {
  const level = getUtilizationLevel(window.utilization)

  return (
    <div className={styles.quotaCard}>
      <div className={styles.quotaLabel}>{label}</div>
      <div className={styles.quotaBarContainer}>
        <div
          className={`${styles.quotaBar} ${styles[level]}`}
          style={{ width: `${Math.min(window.utilization, 100)}%` }}
        />
      </div>
      <div className={styles.quotaInfo}>
        <span className={`${styles.quotaPercent} ${styles[level]}`}>
          {window.utilization.toFixed(1)}%
        </span>
        <span className={styles.quotaReset}>
          {formatResetTime(window.resets_at)}
        </span>
      </div>
    </div>
  )
}

export const UsagePanel = memo(function UsagePanel() {
  const [viewType, setViewType] = useState<ViewType>('daily')
  const [dailyData, setDailyData] = useState<DailyUsage[]>([])
  const [monthlyData, setMonthlyData] = useState<MonthlyUsage[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  // Quota state
  const [quota, setQuota] = useState<QuotaData | null>(null)
  const [quotaLoading, setQuotaLoading] = useState(true)
  const [quotaError, setQuotaError] = useState<string | null>(null)

  // Load usage data
  useEffect(() => {
    async function loadData() {
      setLoading(true)
      setError(null)

      try {
        const [dailyRes, monthlyRes] = await Promise.all([
          api.fetchUsage('daily'),
          api.fetchUsage('monthly'),
        ])

        if (dailyRes.success && dailyRes.data?.daily) {
          setDailyData(dailyRes.data.daily.sort((a, b) => b.date.localeCompare(a.date)))
        }
        if (monthlyRes.success && monthlyRes.data?.monthly) {
          setMonthlyData(monthlyRes.data.monthly.sort((a, b) => b.month.localeCompare(a.month)))
        }
      } catch (err) {
        setError('Failed to load usage data')
      } finally {
        setLoading(false)
      }
    }

    loadData()
  }, [])

  // Load quota data
  const loadQuota = useCallback(async () => {
    setQuotaLoading(true)
    setQuotaError(null)

    try {
      const res = await api.fetchQuota()
      if (res.success && res.data) {
        setQuota(res.data)
      } else {
        setQuotaError(res.error || 'Failed to load quota')
      }
    } catch {
      setQuotaError('Failed to load quota')
    } finally {
      setQuotaLoading(false)
    }
  }, [])

  useEffect(() => {
    loadQuota()
    // Refresh quota every 60 seconds
    const interval = setInterval(loadQuota, 60000)
    return () => clearInterval(interval)
  }, [loadQuota])

  // Calculate totals
  const dailyTotals = dailyData.reduce(
    (acc, day) => ({
      tokens: acc.tokens + day.totalTokens,
      cost: acc.cost + day.totalCost,
    }),
    { tokens: 0, cost: 0 }
  )

  const monthlyTotals = monthlyData.reduce(
    (acc, month) => ({
      tokens: acc.tokens + month.totalTokens,
      cost: acc.cost + month.totalCost,
    }),
    { tokens: 0, cost: 0 }
  )

  if (loading) {
    return (
      <div className={styles.loading}>
        <Spinner size="md" />
        <span>Loading usage data...</span>
      </div>
    )
  }

  if (error) {
    return <div className={styles.error}>{error}</div>
  }

  return (
    <div className={styles.container}>
      {/* Quota Section */}
      <div className={styles.quotaSection}>
        <div className={styles.quotaHeader}>
          <h3 className={styles.quotaTitle}>Rate Limit Quota</h3>
          <button className={styles.quotaRefresh} onClick={loadQuota} disabled={quotaLoading}>
            {quotaLoading ? 'Loading...' : 'Refresh'}
          </button>
        </div>

        {quotaLoading && !quota && (
          <div className={styles.quotaLoading}>
            <Spinner size="sm" />
            <span>Loading quota...</span>
          </div>
        )}

        {quotaError && !quota && (
          <div className={styles.quotaError}>{quotaError}</div>
        )}

        {quota && (
          <div className={styles.quotaGrid}>
            <QuotaBar window={quota.five_hour} label="5-Hour Window" />
            <QuotaBar window={quota.seven_day} label="7-Day Window" />
            {quota.seven_day_sonnet && quota.seven_day_sonnet.utilization > 0 && (
              <QuotaBar window={quota.seven_day_sonnet} label="Sonnet (7-Day)" />
            )}
            {quota.seven_day_opus && quota.seven_day_opus.utilization > 0 && (
              <QuotaBar window={quota.seven_day_opus} label="Opus (7-Day)" />
            )}
          </div>
        )}
      </div>

      {/* Usage Analytics Header */}
      <div className={styles.header}>
        <h3 className={styles.title}>Usage Analytics (via ccusage)</h3>
        <div className={styles.viewToggle}>
          <button
            className={`${styles.toggleBtn} ${viewType === 'daily' ? styles.active : ''}`}
            onClick={() => setViewType('daily')}
          >
            Daily
          </button>
          <button
            className={`${styles.toggleBtn} ${viewType === 'monthly' ? styles.active : ''}`}
            onClick={() => setViewType('monthly')}
          >
            Monthly
          </button>
        </div>
      </div>

      {/* Summary */}
      <div className={styles.summary}>
        <div className={styles.summaryCard}>
          <span className={styles.summaryLabel}>Total Tokens</span>
          <span className={styles.summaryValue}>
            {formatTokens(viewType === 'daily' ? dailyTotals.tokens : monthlyTotals.tokens)}
          </span>
        </div>
        <div className={styles.summaryCard}>
          <span className={styles.summaryLabel}>Total Cost</span>
          <span className={styles.summaryValue}>
            {formatCost(viewType === 'daily' ? dailyTotals.cost : monthlyTotals.cost)}
          </span>
        </div>
        <div className={styles.summaryCard}>
          <span className={styles.summaryLabel}>
            {viewType === 'daily' ? 'Active Days' : 'Active Months'}
          </span>
          <span className={styles.summaryValue}>
            {viewType === 'daily' ? dailyData.length : monthlyData.length}
          </span>
        </div>
      </div>

      {/* Table */}
      <div className={styles.tableWrapper}>
        <table className={styles.table}>
          <thead>
            <tr>
              <th>{viewType === 'daily' ? 'Date' : 'Month'}</th>
              <th>Input</th>
              <th>Output</th>
              <th>Cache Write</th>
              <th>Cache Read</th>
              <th>Total Tokens</th>
              <th>Cost</th>
              <th>Models</th>
            </tr>
          </thead>
          <tbody>
            {viewType === 'daily' ? (
              dailyData.map((day) => (
                <tr key={day.date}>
                  <td className={styles.dateCell}>{day.date}</td>
                  <td>{formatTokens(day.inputTokens)}</td>
                  <td>{formatTokens(day.outputTokens)}</td>
                  <td>{formatTokens(day.cacheCreationTokens)}</td>
                  <td>{formatTokens(day.cacheReadTokens)}</td>
                  <td className={styles.totalCell}>{formatTokens(day.totalTokens)}</td>
                  <td className={styles.costCell}>{formatCost(day.totalCost)}</td>
                  <td className={styles.modelsCell}>
                    {day.modelsUsed.map(m => m.replace('claude-', '').split('-').slice(0, 2).join('-')).join(', ')}
                  </td>
                </tr>
              ))
            ) : (
              monthlyData.map((month) => (
                <tr key={month.month}>
                  <td className={styles.dateCell}>{month.month}</td>
                  <td>{formatTokens(month.inputTokens)}</td>
                  <td>{formatTokens(month.outputTokens)}</td>
                  <td>{formatTokens(month.cacheCreationTokens)}</td>
                  <td>{formatTokens(month.cacheReadTokens)}</td>
                  <td className={styles.totalCell}>{formatTokens(month.totalTokens)}</td>
                  <td className={styles.costCell}>{formatCost(month.totalCost)}</td>
                  <td className={styles.modelsCell}>
                    {month.modelsUsed.map(m => m.replace('claude-', '').split('-').slice(0, 2).join('-')).join(', ')}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {dailyData.length === 0 && monthlyData.length === 0 && (
        <div className={styles.empty}>No usage data available</div>
      )}
    </div>
  )
})
