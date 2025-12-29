import { useState, useEffect, useCallback, memo, useMemo } from 'react'
import type { ClientDefinition, ClientQuotaWindow, CodexQuotaData, DailyUsage, MonthlyUsage, QuotaData, QuotaWindow } from '@/types'
import { api } from '@/services/api'
import { Spinner } from '@/components/Spinner'
import { formatCost, formatTokens } from '@/utils/format'
import styles from './UsagePanel.module.css'

type ViewType = 'daily' | 'monthly'

type ClientStatus = 'connected' | 'checking' | 'needs-setup' | 'custom'

const DEFAULT_CLIENTS: ClientDefinition[] = [
  { id: 'claude', name: 'Claude Code', kind: 'cli', status: 'connected' },
  { id: 'codex', name: 'Codex CLI', kind: 'cli', status: 'needs-setup' },
  { id: 'gemini', name: 'Gemini CLI', kind: 'cli', status: 'needs-setup' },
  { id: 'cursor', name: 'Cursor', kind: 'ide', status: 'needs-setup' },
  { id: 'opencode', name: 'OpenCode', kind: 'cli', status: 'needs-setup' },
]

const CLIENT_BADGES: Record<string, string> = {
  claude: 'CL',
  codex: 'CX',
  gemini: 'GM',
  cursor: 'CR',
  opencode: 'OC',
}

function normalizeClientStatus(status?: string): ClientStatus {
  if (status === 'connected') return 'connected'
  if (status === 'checking') return 'checking'
  if (status === 'custom') return 'custom'
  return 'needs-setup'
}

function getClientBadge(client: ClientDefinition): string {
  return CLIENT_BADGES[client.id] || client.name.slice(0, 2).toUpperCase()
}

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
  const percentage = Math.min(window.utilization, 100)

  return (
    <div className={styles.quotaCard}>
      <div className={styles.quotaCardHeader}>
        <span className={styles.quotaLabel}>{label}</span>
        <span className={`${styles.quotaPercent} ${styles[level]}`}>
          {window.utilization.toFixed(0)}%
        </span>
      </div>
      <div className={styles.quotaBarContainer}>
        <div
          className={`${styles.quotaBar} ${styles[level]}`}
          style={{ width: `${percentage}%` }}
        />
      </div>
      <div className={styles.quotaReset}>
        {formatResetTime(window.resets_at)}
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

  // Client state
  const [activeClientId, setActiveClientId] = useState('claude')
  const [clientDefs, setClientDefs] = useState<ClientDefinition[]>([])
  const [clientsLoading, setClientsLoading] = useState(true)
  const [clientsError, setClientsError] = useState<string | null>(null)
  const [clientsSourcePath, setClientsSourcePath] = useState<string | null>(null)

  // Codex quota state
  const [codexQuota, setCodexQuota] = useState<CodexQuotaData | null>(null)
  const [codexLoading, setCodexLoading] = useState(false)
  const [codexError, setCodexError] = useState<string | null>(null)

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

  const loadClients = useCallback(async () => {
    setClientsLoading(true)
    setClientsError(null)

    try {
      const res = await api.fetchClients()
      if (res.success && res.data) {
        setClientDefs(res.data.clients || [])
        setClientsSourcePath(res.data.source_path ?? null)
      } else {
        setClientsError(res.error || 'Failed to load clients')
      }
    } catch {
      setClientsError('Failed to load clients')
    } finally {
      setClientsLoading(false)
    }
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

  const loadCodexQuota = useCallback(async () => {
    setCodexLoading(true)
    setCodexError(null)

    try {
      const res = await api.fetchCodexQuota()
      if (res.success && res.data) {
        setCodexQuota(res.data)
      } else {
        setCodexError(res.error || 'Failed to load Codex quota')
      }
    } catch {
      setCodexError('Failed to load Codex quota')
    } finally {
      setCodexLoading(false)
    }
  }, [])

  useEffect(() => {
    loadQuota()
    // Refresh quota every 60 seconds
    const interval = setInterval(loadQuota, 60000)
    return () => clearInterval(interval)
  }, [loadQuota])

  useEffect(() => {
    if (activeClientId !== 'codex') return
    loadCodexQuota()
    const interval = setInterval(loadCodexQuota, 60000)
    return () => clearInterval(interval)
  }, [activeClientId, loadCodexQuota])

  useEffect(() => {
    loadClients()
  }, [loadClients])

  const mergedClients = useMemo(() => {
    const customMap = new Map(clientDefs.map(client => [client.id, client]))
    const merged = DEFAULT_CLIENTS.map(client => ({
      ...client,
      ...customMap.get(client.id),
    }))
    for (const customClient of clientDefs) {
      if (!merged.find(client => client.id === customClient.id)) {
        merged.push(customClient)
      }
    }
    const claudeStatus: ClientStatus = quotaLoading ? 'checking' : quota ? 'connected' : 'needs-setup'
    return merged.map(client => {
      if (client.id !== 'claude') return client
      return {
        ...client,
        status: claudeStatus,
      }
    })
  }, [clientDefs, quota, quotaLoading])

  const clientsWithCodex = useMemo(() => {
    return mergedClients.map(client => {
      if (client.id !== 'codex') return client
      const status: ClientStatus = codexLoading ? 'checking' : codexQuota ? 'connected' : 'needs-setup'
      return {
        ...client,
        status,
      }
    })
  }, [mergedClients, codexLoading, codexQuota])

  const activeClient = clientsWithCodex.find(client => client.id === activeClientId) || clientsWithCodex[0]
  const activeCustomQuota = activeClient?.quota_windows ?? []
  const hasCustomQuota = activeCustomQuota.length > 0

  useEffect(() => {
    if (!clientsWithCodex.length) return
    const stillExists = clientsWithCodex.some(client => client.id === activeClientId)
    if (!stillExists) {
      setActiveClientId(clientsWithCodex[0].id)
    }
  }, [activeClientId, clientsWithCodex])

  const isClaudeActive = activeClientId === 'claude'
  const isCodexActive = activeClientId === 'codex'
  const quotaRefreshLoading = isClaudeActive ? quotaLoading : isCodexActive ? codexLoading : clientsLoading

  const handleQuotaRefresh = useCallback(() => {
    if (isClaudeActive) {
      loadQuota()
      return
    }
    if (isCodexActive) {
      loadCodexQuota()
      return
    }
    loadClients()
  }, [isClaudeActive, isCodexActive, loadQuota, loadCodexQuota, loadClients])

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
      {/* Clients Section */}
      <div className={styles.clientSection}>
        <div className={styles.clientHeader}>
          <div>
            <h3 className={styles.clientTitle}>Clients</h3>
            <p className={styles.clientSubtitle}>Switch between supported clients and plug in custom quota sources.</p>
          </div>
          <button
            className={styles.clientReload}
            onClick={loadClients}
            disabled={clientsLoading}
          >
            {clientsLoading ? 'Checking...' : 'Reload'}
          </button>
        </div>

        <div className={styles.clientRow}>
          {clientsWithCodex.map(client => {
            const status = normalizeClientStatus(client.status)
            return (
              <button
                key={client.id}
                className={`${styles.clientChip} ${activeClientId === client.id ? styles.clientChipActive : ''}`}
                onClick={() => setActiveClientId(client.id)}
                type="button"
              >
                <span className={styles.clientBadge}>{getClientBadge(client)}</span>
                <span className={styles.clientInfo}>
                  <span className={styles.clientName}>{client.name}</span>
                  <span className={`${styles.clientStatus} ${styles[`clientStatus_${status}`]}`}>
                    <span className={styles.statusDot} />
                    {status === 'connected' && 'Connected'}
                    {status === 'checking' && 'Checking'}
                    {status === 'custom' && 'Custom'}
                    {status === 'needs-setup' && 'Needs setup'}
                  </span>
                </span>
              </button>
            )
          })}
        </div>

        {clientsError && (
          <div className={styles.clientError}>{clientsError}</div>
        )}

        {clientsSourcePath && (
          <div className={styles.clientMeta}>
            Config: <code>{clientsSourcePath}</code>
          </div>
        )}
      </div>

      {/* Quota Section */}
      <div className={styles.quotaSection}>
        <div className={styles.quotaHeader}>
          <div>
            <h3 className={styles.quotaTitle}>Rate Limit Quota</h3>
            {activeClient?.note && <div className={styles.quotaNote}>{activeClient.note}</div>}
          </div>
          <button
            className={styles.quotaRefresh}
            onClick={handleQuotaRefresh}
            disabled={quotaRefreshLoading}
          >
            {quotaRefreshLoading ? 'Loading...' : 'Refresh'}
          </button>
        </div>

        {isClaudeActive && quotaLoading && !quota && (
          <div className={styles.quotaLoading}>
            <Spinner size="sm" />
            <span>Loading quota...</span>
          </div>
        )}

        {isClaudeActive && quotaError && !quota && (
          <div className={styles.quotaError}>
            {quotaError}
            <br />
            <small>Try: claude /logout then claude /login</small>
          </div>
        )}

        {isClaudeActive && quota && (
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

        {isCodexActive && codexLoading && !codexQuota && (
          <div className={styles.quotaLoading}>
            <Spinner size="sm" />
            <span>Loading Codex quota...</span>
          </div>
        )}

        {isCodexActive && codexError && !codexQuota && (
          <div className={styles.quotaError}>
            {codexError}
            <br />
            <small>Try: codex /logout then codex /login</small>
          </div>
        )}

        {isCodexActive && codexQuota && (
          <div className={styles.quotaGrid}>
            <QuotaBar window={codexQuota.session} label="Session (3-Hour)" />
            <QuotaBar window={codexQuota.weekly} label="Weekly" />
          </div>
        )}

        {!isClaudeActive && !isCodexActive && hasCustomQuota && (
          <div className={styles.quotaGrid}>
            {activeCustomQuota.map((window: ClientQuotaWindow) => (
              <QuotaBar
                key={window.id || window.label}
                window={{ utilization: window.utilization, resets_at: window.resets_at }}
                label={window.label}
              />
            ))}
          </div>
        )}

        {!isClaudeActive && !isCodexActive && !hasCustomQuota && (
          <div className={styles.quotaEmpty}>
            <div className={styles.quotaEmptyTitle}>No quota data for {activeClient?.name || 'this client'} yet.</div>
            <div className={styles.quotaEmptyBody}>
              Add quota_windows to your clients config file to surface usage here.
            </div>
          </div>
        )}
      </div>

      {activeClientId !== 'claude' && (
        <div className={styles.clientNotice}>
          Usage analytics currently track Claude Code only (via ccusage).
          <span> Connect other clients to surface their quota in this view.</span>
        </div>
      )}

      {/* Usage Analytics Header */}
      {activeClientId === 'claude' && (
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
      )}

      {/* Summary */}
      {activeClientId === 'claude' && (
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
      )}

      {/* Table */}
      {activeClientId === 'claude' && (
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
      )}

      {activeClientId === 'claude' && dailyData.length === 0 && monthlyData.length === 0 && (
        <div className={styles.empty}>No usage data available</div>
      )}
    </div>
  )
})
