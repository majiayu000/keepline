import { useState, memo } from 'react'
import type { Session } from '@/types'
import { Button } from '@/components/Button'
import { ResponsePanel } from '@/components/ResponsePanel'
import { ToolCallList } from '@/components/ToolCallList'
import { UsageStats } from '@/components/UsageStats'
import { formatRelativeTime, formatPath } from '@/utils/format'
import { getStatusColor } from '@/constants'
import styles from './SessionCard.module.css'

interface SessionCardProps {
  session: Session
  onRecover?: (sessionId: string) => void
  onStop?: (sessionId: string) => void
  onComplete?: (sessionId: string) => void
}

export const SessionCard = memo(function SessionCard({
  session,
  onRecover,
  onStop,
  onComplete
}: SessionCardProps) {
  const [expanded, setExpanded] = useState(false)

  const statusColor = getStatusColor(session.status)

  return (
    <div className={styles.card} style={{ borderLeftColor: statusColor }}>
      <div className={styles.header} onClick={() => setExpanded(!expanded)}>
        <div className={styles.statusRow}>
          <span className={styles.status} style={{ color: statusColor }}>
            {session.status}
          </span>
          <span className={styles.meta}>
            {session.pid ? `PID: ${session.pid}` : 'No process'}
          </span>
        </div>
        <div className={styles.title}>
          {session.title || '(no title)'}
        </div>
        <div className={styles.path}>{formatPath(session.directory)}</div>
        <div className={styles.stats}>
          <span>Tools: {session.toolCount}</span>
          <span>Messages: {session.messageCount}</span>
          {session.lastTool && <span>Last: {session.lastTool}</span>}
        </div>
        <div className={styles.statsRow}>
          <div className={styles.lastActive}>
            {formatRelativeTime(session.lastActiveAt)}
          </div>
          {session.usageStats && session.usageStats.totalTokens > 0 && (
            <UsageStats stats={session.usageStats} compact />
          )}
        </div>
        <span className={styles.expandIcon}>{expanded ? '▼' : '▶'}</span>
      </div>

      {expanded && (
        <div className={styles.details}>
          {session.initialPrompt && (
            <div className={styles.section}>
              <h4 className={styles.sectionTitle}>Initial Prompt</h4>
              <div className={styles.promptBox}>{session.initialPrompt}</div>
            </div>
          )}

          {session.toolCount > 0 && (
            <div className={styles.section}>
              <h4 className={styles.sectionTitle}>Tool Calls</h4>
              <ToolCallList sessionId={session.sessionId} toolCount={session.toolCount} />
            </div>
          )}

          {session.lastMessage && (
            <div className={styles.section}>
              <h4 className={styles.sectionTitle}>Last Response</h4>
              <ResponsePanel content={session.lastMessage} />
            </div>
          )}

          {session.usageStats && session.usageStats.totalTokens > 0 && (
            <div className={styles.section}>
              <h4 className={styles.sectionTitle}>Usage Statistics</h4>
              <UsageStats stats={session.usageStats} />
            </div>
          )}

          <div className={styles.actions}>
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
    </div>
  )
})
