import { useState, useEffect, memo } from 'react'
import type { DailyUsage, MonthlyUsage } from '@/types'
import { api } from '@/services/api'
import { Spinner } from '@/components/Spinner'
import { formatCost, formatTokens } from '@/utils/format'
import styles from './UsagePanel.module.css'

type ViewType = 'daily' | 'monthly'

export const UsagePanel = memo(function UsagePanel() {
  const [viewType, setViewType] = useState<ViewType>('daily')
  const [dailyData, setDailyData] = useState<DailyUsage[]>([])
  const [monthlyData, setMonthlyData] = useState<MonthlyUsage[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

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
          // Sort by date descending (newest first)
          setDailyData(dailyRes.data.daily.sort((a, b) => b.date.localeCompare(a.date)))
        }
        if (monthlyRes.success && monthlyRes.data?.monthly) {
          // Sort by month descending
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
