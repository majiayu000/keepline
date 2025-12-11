import { useState, useEffect, memo } from 'react'
import type { SubAgent, SubAgentsData } from '@/types'
import { api } from '@/services/api'
import { formatRelativeTime, formatTokens } from '@/utils/format'
import styles from './SubAgentList.module.css'

interface SubAgentListProps {
  sessionId: string
  // Cached data from parent (from /full endpoint) - avoids separate API call
  cachedData?: SubAgentsData
}

export const SubAgentList = memo(function SubAgentList({ sessionId, cachedData }: SubAgentListProps) {
  const [subAgents, setSubAgents] = useState<SubAgent[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [expanded, setExpanded] = useState<Record<string, boolean>>({})

  // Use cached data if available, otherwise fetch
  useEffect(() => {
    // If we have cached data from parent, use it directly (no API call needed)
    if (cachedData) {
      setSubAgents(cachedData.subAgents)
      setLoading(false)
      return
    }

    // Fallback: fetch if no cached data (backwards compatibility)
    let mounted = true

    async function loadSubAgents() {
      setLoading(true)
      setError(null)

      const response = await api.fetchSubAgents(sessionId)

      if (!mounted) return

      if (response.success && response.data) {
        setSubAgents(response.data.subAgents)
      } else {
        setError(response.error || 'Failed to load sub-agents')
      }
      setLoading(false)
    }

    loadSubAgents()

    return () => {
      mounted = false
    }
  }, [sessionId, cachedData])

  const toggleExpand = (agentId: string) => {
    setExpanded(prev => ({
      ...prev,
      [agentId]: !prev[agentId],
    }))
  }

  if (loading) {
    return <div className={styles.loading}>Loading sub-agents...</div>
  }

  if (error) {
    return <div className={styles.error}>{error}</div>
  }

  if (subAgents.length === 0) {
    return <div className={styles.empty}>No sub-agents spawned</div>
  }

  return (
    <div className={styles.container}>
      <div className={styles.header}>
        <span className={styles.icon}>🤖</span>
        <span className={styles.count}>{subAgents.length} sub-agent{subAgents.length !== 1 ? 's' : ''}</span>
      </div>
      <ul className={styles.list}>
        {subAgents.map((agent) => {
          const isExpanded = expanded[agent.sessionId]
          return (
            <li key={agent.sessionId} className={styles.item}>
              <div
                className={styles.agentHeader}
                onClick={() => toggleExpand(agent.sessionId)}
                role="button"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault()
                    toggleExpand(agent.sessionId)
                  }
                }}
              >
                <span className={styles.expandIcon}>{isExpanded ? '▼' : '▶'}</span>
                <span className={styles.agentId}>{agent.agentId || 'unknown'}</span>
                <span className={styles.agentStats}>
                  {agent.toolCount} tools, {agent.messageCount} msgs
                </span>
                <span className={styles.agentTime}>
                  {formatRelativeTime(agent.lastActiveAt)}
                </span>
              </div>
              {isExpanded && (
                <div className={styles.agentDetails}>
                  {agent.firstMessage && (
                    <div className={styles.field}>
                      <span className={styles.fieldLabel}>Task:</span>
                      <span className={styles.fieldValue}>{agent.firstMessage.slice(0, 200)}{agent.firstMessage.length > 200 ? '...' : ''}</span>
                    </div>
                  )}
                  {agent.lastTool && (
                    <div className={styles.field}>
                      <span className={styles.fieldLabel}>Last Tool:</span>
                      <span className={styles.fieldValue}>{agent.lastTool}</span>
                    </div>
                  )}
                  {agent.usageStats && (
                    <div className={styles.field}>
                      <span className={styles.fieldLabel}>Tokens:</span>
                      <span className={styles.fieldValue}>
                        {formatTokens(agent.usageStats.totalTokens)}
                        {agent.usageStats.totalCost > 0 && ` ($${agent.usageStats.totalCost.toFixed(4)})`}
                      </span>
                    </div>
                  )}
                  {agent.lastMessage && (
                    <div className={styles.field}>
                      <span className={styles.fieldLabel}>Last Response:</span>
                      <div className={styles.lastMessage}>
                        {agent.lastMessage.slice(0, 300)}{agent.lastMessage.length > 300 ? '...' : ''}
                      </div>
                    </div>
                  )}
                </div>
              )}
            </li>
          )
        })}
      </ul>
    </div>
  )
})
