import { memo, useMemo, useState } from 'react'
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  PieChart,
  Pie,
  Cell,
  BarChart,
  Bar,
} from 'recharts'
import type { Session } from '@/types'
import { formatTokens, formatCost } from '@/utils/format'
import styles from './AnalyticsPanel.module.css'

interface AnalyticsPanelProps {
  sessions: Session[]
}

type TabType = 'timeline' | 'tools' | 'status'

// Theme-aware colors
const CHART_COLORS = [
  '#00f0ff', // cyan
  '#ff00ff', // magenta
  '#00ff88', // green
  '#ffaa00', // orange
  '#ff4444', // red
  '#8844ff', // purple
  '#44ff44', // lime
  '#ff8888', // pink
]

const STATUS_CHART_COLORS: Record<string, string> = {
  running: '#00ff88',
  waiting: '#ffaa00',
  idle: '#00f0ff',
  lost: '#ff4444',
  completed: '#888888',
}

export const AnalyticsPanel = memo(function AnalyticsPanel({ sessions }: AnalyticsPanelProps) {
  const [collapsed, setCollapsed] = useState(false)
  const [activeTab, setActiveTab] = useState<TabType>('timeline')

  // Prepare timeline data (sessions grouped by day)
  const timelineData = useMemo(() => {
    const byDate: Record<string, { date: string; tokens: number; cost: number; sessions: number }> = {}

    for (const session of sessions) {
      const date = session.lastActiveAt?.split('T')[0] || session.createdAt?.split('T')[0]
      if (!date) continue

      if (!byDate[date]) {
        byDate[date] = { date, tokens: 0, cost: 0, sessions: 0 }
      }

      byDate[date].sessions++
      if (session.usageStats) {
        byDate[date].tokens += session.usageStats.totalTokens || 0
        byDate[date].cost += session.usageStats.totalCost || 0
      }
    }

    return Object.values(byDate)
      .sort((a, b) => a.date.localeCompare(b.date))
      .slice(-14) // Last 14 days
  }, [sessions])

  // Prepare tool usage data
  const toolData = useMemo(() => {
    const toolCounts: Record<string, number> = {}

    for (const session of sessions) {
      if (session.lastTool) {
        toolCounts[session.lastTool] = (toolCounts[session.lastTool] || 0) + 1
      }
    }

    return Object.entries(toolCounts)
      .map(([name, count]) => ({ name, count }))
      .sort((a, b) => b.count - a.count)
      .slice(0, 8) // Top 8 tools
  }, [sessions])

  // Prepare status distribution data
  const statusData = useMemo(() => {
    const statusCounts: Record<string, { count: number; tokens: number; cost: number }> = {}

    for (const session of sessions) {
      const status = session.status
      if (!statusCounts[status]) {
        statusCounts[status] = { count: 0, tokens: 0, cost: 0 }
      }
      statusCounts[status].count++
      if (session.usageStats) {
        statusCounts[status].tokens += session.usageStats.totalTokens || 0
        statusCounts[status].cost += session.usageStats.totalCost || 0
      }
    }

    return Object.entries(statusCounts).map(([status, data]) => ({
      name: status.charAt(0).toUpperCase() + status.slice(1),
      status,
      ...data,
    }))
  }, [sessions])

  if (sessions.length === 0) {
    return null
  }

  const CustomTooltip = ({ active, payload, label }: any) => {
    if (!active || !payload?.length) return null

    return (
      <div style={{
        background: 'var(--bg-surface)',
        border: '1px solid var(--border)',
        borderRadius: '6px',
        padding: '0.75rem',
        fontSize: '0.8rem',
      }}>
        <p style={{ margin: '0 0 0.5rem', color: 'var(--text)', fontWeight: 600 }}>{label}</p>
        {payload.map((entry: any, index: number) => (
          <p key={index} style={{ margin: '0.25rem 0', color: entry.color }}>
            {entry.name}: {entry.name === 'Cost' ? formatCost(entry.value) : formatTokens(entry.value)}
          </p>
        ))}
      </div>
    )
  }

  const PieTooltip = ({ active, payload }: any) => {
    if (!active || !payload?.length) return null

    const data = payload[0]
    return (
      <div style={{
        background: 'var(--bg-surface)',
        border: '1px solid var(--border)',
        borderRadius: '6px',
        padding: '0.75rem',
        fontSize: '0.8rem',
      }}>
        <p style={{ margin: 0, color: data.payload.fill, fontWeight: 600 }}>
          {data.name}: {data.value}
        </p>
      </div>
    )
  }

  return (
    <div className={styles.panel}>
      <div className={styles.header} onClick={() => setCollapsed(!collapsed)}>
        <div className={styles.headerLeft}>
          <span className={styles.icon}>📊</span>
          <h3 className={styles.title}>Analytics</h3>
        </div>
        <span className={`${styles.collapseIcon} ${collapsed ? styles.collapsed : ''}`}>▼</span>
      </div>

      {!collapsed && (
        <div className={styles.content}>
          <div className={styles.tabs}>
            <button
              className={`${styles.tab} ${activeTab === 'timeline' ? styles.active : ''}`}
              onClick={() => setActiveTab('timeline')}
            >
              Timeline
            </button>
            <button
              className={`${styles.tab} ${activeTab === 'tools' ? styles.active : ''}`}
              onClick={() => setActiveTab('tools')}
            >
              Tools
            </button>
            <button
              className={`${styles.tab} ${activeTab === 'status' ? styles.active : ''}`}
              onClick={() => setActiveTab('status')}
            >
              Status
            </button>
          </div>

          {activeTab === 'timeline' && (
            <div className={styles.grid}>
              <div className={styles.chartCard}>
                <h4 className={styles.chartTitle}>Token Usage Over Time</h4>
                {timelineData.length > 0 ? (
                  <div className={styles.chartContainer}>
                    <ResponsiveContainer width="100%" height="100%">
                      <AreaChart data={timelineData}>
                        <defs>
                          <linearGradient id="tokenGradient" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="5%" stopColor="#00f0ff" stopOpacity={0.3} />
                            <stop offset="95%" stopColor="#00f0ff" stopOpacity={0} />
                          </linearGradient>
                        </defs>
                        <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
                        <XAxis
                          dataKey="date"
                          stroke="var(--text-dim)"
                          fontSize={11}
                          tickFormatter={(value) => value.slice(5)} // MM-DD
                        />
                        <YAxis
                          stroke="var(--text-dim)"
                          fontSize={11}
                          tickFormatter={(value) => formatTokens(value)}
                        />
                        <Tooltip content={<CustomTooltip />} />
                        <Area
                          type="monotone"
                          dataKey="tokens"
                          name="Tokens"
                          stroke="#00f0ff"
                          fill="url(#tokenGradient)"
                          strokeWidth={2}
                        />
                      </AreaChart>
                    </ResponsiveContainer>
                  </div>
                ) : (
                  <div className={styles.emptyState}>
                    <span className={styles.emptyIcon}>📈</span>
                    <span>No timeline data available</span>
                  </div>
                )}
              </div>

              <div className={styles.chartCard}>
                <h4 className={styles.chartTitle}>Cost Over Time</h4>
                {timelineData.length > 0 ? (
                  <div className={styles.chartContainer}>
                    <ResponsiveContainer width="100%" height="100%">
                      <AreaChart data={timelineData}>
                        <defs>
                          <linearGradient id="costGradient" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="5%" stopColor="#ff00ff" stopOpacity={0.3} />
                            <stop offset="95%" stopColor="#ff00ff" stopOpacity={0} />
                          </linearGradient>
                        </defs>
                        <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
                        <XAxis
                          dataKey="date"
                          stroke="var(--text-dim)"
                          fontSize={11}
                          tickFormatter={(value) => value.slice(5)}
                        />
                        <YAxis
                          stroke="var(--text-dim)"
                          fontSize={11}
                          tickFormatter={(value) => `$${value.toFixed(2)}`}
                        />
                        <Tooltip content={<CustomTooltip />} />
                        <Area
                          type="monotone"
                          dataKey="cost"
                          name="Cost"
                          stroke="#ff00ff"
                          fill="url(#costGradient)"
                          strokeWidth={2}
                        />
                      </AreaChart>
                    </ResponsiveContainer>
                  </div>
                ) : (
                  <div className={styles.emptyState}>
                    <span className={styles.emptyIcon}>💰</span>
                    <span>No cost data available</span>
                  </div>
                )}
              </div>
            </div>
          )}

          {activeTab === 'tools' && (
            <div className={styles.grid}>
              <div className={styles.chartCard}>
                <h4 className={styles.chartTitle}>Tool Usage Distribution</h4>
                {toolData.length > 0 ? (
                  <>
                    <div className={styles.chartContainer}>
                      <ResponsiveContainer width="100%" height="100%">
                        <PieChart>
                          <Pie
                            data={toolData}
                            cx="50%"
                            cy="50%"
                            innerRadius={60}
                            outerRadius={100}
                            paddingAngle={2}
                            dataKey="count"
                            nameKey="name"
                          >
                            {toolData.map((_, index) => (
                              <Cell
                                key={`cell-${index}`}
                                fill={CHART_COLORS[index % CHART_COLORS.length]}
                              />
                            ))}
                          </Pie>
                          <Tooltip content={<PieTooltip />} />
                        </PieChart>
                      </ResponsiveContainer>
                    </div>
                    <div className={styles.legendContainer}>
                      {toolData.map((item, index) => (
                        <div key={item.name} className={styles.legendItem}>
                          <span
                            className={styles.legendDot}
                            style={{ backgroundColor: CHART_COLORS[index % CHART_COLORS.length] }}
                          />
                          <span>{item.name} ({item.count})</span>
                        </div>
                      ))}
                    </div>
                  </>
                ) : (
                  <div className={styles.emptyState}>
                    <span className={styles.emptyIcon}>🔧</span>
                    <span>No tool usage data</span>
                  </div>
                )}
              </div>

              <div className={styles.chartCard}>
                <h4 className={styles.chartTitle}>Top Tools by Usage</h4>
                {toolData.length > 0 ? (
                  <div className={styles.chartContainer}>
                    <ResponsiveContainer width="100%" height="100%">
                      <BarChart data={toolData} layout="vertical">
                        <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
                        <XAxis type="number" stroke="var(--text-dim)" fontSize={11} />
                        <YAxis
                          type="category"
                          dataKey="name"
                          stroke="var(--text-dim)"
                          fontSize={11}
                          width={80}
                        />
                        <Tooltip content={<PieTooltip />} />
                        <Bar dataKey="count" name="Usage">
                          {toolData.map((_, index) => (
                            <Cell
                              key={`bar-${index}`}
                              fill={CHART_COLORS[index % CHART_COLORS.length]}
                            />
                          ))}
                        </Bar>
                      </BarChart>
                    </ResponsiveContainer>
                  </div>
                ) : (
                  <div className={styles.emptyState}>
                    <span className={styles.emptyIcon}>📊</span>
                    <span>No data available</span>
                  </div>
                )}
              </div>
            </div>
          )}

          {activeTab === 'status' && (
            <div className={styles.grid}>
              <div className={styles.chartCard}>
                <h4 className={styles.chartTitle}>Sessions by Status</h4>
                {statusData.length > 0 ? (
                  <>
                    <div className={styles.chartContainer}>
                      <ResponsiveContainer width="100%" height="100%">
                        <PieChart>
                          <Pie
                            data={statusData}
                            cx="50%"
                            cy="50%"
                            innerRadius={60}
                            outerRadius={100}
                            paddingAngle={2}
                            dataKey="count"
                            nameKey="name"
                          >
                            {statusData.map((item) => (
                              <Cell
                                key={`cell-${item.status}`}
                                fill={STATUS_CHART_COLORS[item.status] || '#888888'}
                              />
                            ))}
                          </Pie>
                          <Tooltip content={<PieTooltip />} />
                        </PieChart>
                      </ResponsiveContainer>
                    </div>
                    <div className={styles.legendContainer}>
                      {statusData.map((item) => (
                        <div key={item.status} className={styles.legendItem}>
                          <span
                            className={styles.legendDot}
                            style={{ backgroundColor: STATUS_CHART_COLORS[item.status] || '#888888' }}
                          />
                          <span>{item.name} ({item.count})</span>
                        </div>
                      ))}
                    </div>
                  </>
                ) : (
                  <div className={styles.emptyState}>
                    <span className={styles.emptyIcon}>📋</span>
                    <span>No status data</span>
                  </div>
                )}
              </div>

              <div className={styles.chartCard}>
                <h4 className={styles.chartTitle}>Cost by Status</h4>
                {statusData.length > 0 ? (
                  <div className={styles.chartContainer}>
                    <ResponsiveContainer width="100%" height="100%">
                      <BarChart data={statusData}>
                        <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
                        <XAxis dataKey="name" stroke="var(--text-dim)" fontSize={11} />
                        <YAxis
                          stroke="var(--text-dim)"
                          fontSize={11}
                          tickFormatter={(value) => `$${value.toFixed(2)}`}
                        />
                        <Tooltip
                          content={({ active, payload }) => {
                            if (!active || !payload?.length) return null
                            const data = payload[0].payload
                            return (
                              <div style={{
                                background: 'var(--bg-surface)',
                                border: '1px solid var(--border)',
                                borderRadius: '6px',
                                padding: '0.75rem',
                                fontSize: '0.8rem',
                              }}>
                                <p style={{ margin: '0 0 0.5rem', color: 'var(--text)', fontWeight: 600 }}>
                                  {data.name}
                                </p>
                                <p style={{ margin: '0.25rem 0', color: 'var(--text-dim)' }}>
                                  Sessions: {data.count}
                                </p>
                                <p style={{ margin: '0.25rem 0', color: 'var(--primary)' }}>
                                  Cost: {formatCost(data.cost)}
                                </p>
                                <p style={{ margin: '0.25rem 0', color: 'var(--info)' }}>
                                  Tokens: {formatTokens(data.tokens)}
                                </p>
                              </div>
                            )
                          }}
                        />
                        <Bar dataKey="cost" name="Cost">
                          {statusData.map((item) => (
                            <Cell
                              key={`bar-${item.status}`}
                              fill={STATUS_CHART_COLORS[item.status] || '#888888'}
                            />
                          ))}
                        </Bar>
                      </BarChart>
                    </ResponsiveContainer>
                  </div>
                ) : (
                  <div className={styles.emptyState}>
                    <span className={styles.emptyIcon}>💰</span>
                    <span>No cost data</span>
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  )
})
