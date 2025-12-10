import { useCallback, useEffect, memo } from 'react'
import type { Session, SessionDetailsData } from '@/types'
import { Button } from '@/components/Button'
import { ResponsePanel } from '@/components/ResponsePanel'
import { ToolCallList } from '@/components/ToolCallList'
import { UsageStats } from '@/components/UsageStats'
import { formatRelativeTime, formatPath } from '@/utils/format'
import { getStatusColor } from '@/constants'
import { useToggle } from '@/hooks'
import styles from './SessionCard.module.css'

interface SessionCardProps {
  session: Session
  onRecover?: (sessionId: string) => void
  onStop?: (sessionId: string) => void
  onComplete?: (sessionId: string) => void
  // Lazy loading
  getSessionDetails?: (sessionId: string) => SessionDetailsData | undefined
  loadSessionDetails?: (sessionId: string) => Promise<SessionDetailsData | null>
  isLoadingDetails?: (sessionId: string) => boolean
}

export const SessionCard = memo(function SessionCard({
  session,
  onRecover,
  onStop,
  onComplete,
  getSessionDetails,
  loadSessionDetails,
  isLoadingDetails,
}: SessionCardProps) {
  const [expanded, toggle] = useToggle(false)

  const statusColor = getStatusColor(session.status)
  const cardId = `session-${session.sessionId}`
  const detailsId = `${cardId}-details`

  // Get cached details or loading state
  const details = getSessionDetails?.(session.sessionId)
  const loadingDetails = isLoadingDetails?.(session.sessionId) ?? false

  // Load details when expanded
  useEffect(() => {
    if (expanded && !details && !loadingDetails && loadSessionDetails) {
      loadSessionDetails(session.sessionId)
    }
  }, [expanded, details, loadingDetails, loadSessionDetails, session.sessionId])

  // Merge session data with lazy-loaded details
  const initialPrompt = details?.initialPrompt ?? session.initialPrompt
  const lastMessage = details?.lastMessage ?? session.lastMessage
  const lastTool = details?.lastTool ?? session.lastTool
  const usageStats = details?.usageStats ?? session.usageStats

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      toggle()
    }
  }, [toggle])

  return (
    <article
      className={styles.card}
      style={{ borderLeftColor: statusColor }}
      aria-labelledby={`${cardId}-title`}
    >
      <div
        className={styles.header}
        onClick={toggle}
        onKeyDown={handleKeyDown}
        role="button"
        tabIndex={0}
        aria-expanded={expanded}
        aria-controls={detailsId}
      >
        <div className={styles.statusRow}>
          <span className={styles.status} style={{ color: statusColor }}>
            {session.status}
          </span>
          <span className={styles.meta}>
            {session.pid ? `PID: ${session.pid}` : 'No process'}
          </span>
        </div>
        <div className={styles.title} id={`${cardId}-title`}>
          {session.title || '(no title)'}
        </div>
        <div className={styles.path}>{formatPath(session.directory)}</div>
        <div className={styles.stats}>
          <span>Tools: {session.toolCount}</span>
          <span>Messages: {session.messageCount}</span>
          {lastTool && <span>Last: {lastTool}</span>}
        </div>
        <div className={styles.statsRow}>
          <div className={styles.lastActive}>
            {formatRelativeTime(session.lastActiveAt)}
          </div>
          {usageStats && usageStats.totalTokens > 0 && (
            <UsageStats stats={usageStats} compact />
          )}
        </div>
        <span className={styles.expandIcon} aria-hidden="true">
          {expanded ? '▼' : '▶'}
        </span>
      </div>

      {expanded && (
        <div className={styles.details} id={detailsId}>
          {loadingDetails && (
            <div className={styles.loading}>Loading details...</div>
          )}

          {initialPrompt && (
            <section className={styles.section}>
              <h4 className={styles.sectionTitle}>Initial Prompt</h4>
              <div className={styles.promptBox}>{initialPrompt}</div>
            </section>
          )}

          {session.toolCount > 0 && (
            <section className={styles.section}>
              <h4 className={styles.sectionTitle}>Tool Calls</h4>
              <ToolCallList sessionId={session.sessionId} toolCount={session.toolCount} />
            </section>
          )}

          {lastMessage && (
            <section className={styles.section}>
              <h4 className={styles.sectionTitle}>Last Response</h4>
              <ResponsePanel content={lastMessage} />
            </section>
          )}

          {usageStats && usageStats.totalTokens > 0 && (
            <section className={styles.section}>
              <h4 className={styles.sectionTitle}>Usage Statistics</h4>
              <UsageStats stats={usageStats} />
            </section>
          )}

          <div className={styles.actions} role="group" aria-label="Session actions">
            {session.status === 'lost' && onRecover && (
              <Button variant="success" size="sm" onClick={() => onRecover(session.sessionId)}>
                Recover
              </Button>
            )}
            {(session.status === 'running' || session.status === 'waiting') && onStop && (
              <Button variant="danger" size="sm" onClick={() => onStop(session.sessionId)}>
                Stop
              </Button>
            )}
            {session.status !== 'completed' && onComplete && (
              <Button variant="secondary" size="sm" onClick={() => onComplete(session.sessionId)}>
                Complete
              </Button>
            )}
          </div>
        </div>
      )}
    </article>
  )
})
