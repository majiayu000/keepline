import { useState, useMemo, memo, useCallback, useEffect } from 'react'
import type { Session, SessionFullData, PaginationInfo, TerminalApp, SessionStatus } from '@/types'
import { SessionCard } from '@/components/SessionCard'
import { Button } from '@/components/Button'
import { STATUS_ORDER } from '@/constants'
import { SESSION_STATUS_PRESENTATION } from '../../../../../domain/session/status-presentation'
import styles from './SessionList.module.css'

interface SessionListProps {
  sessions: Session[]
  onRecover?: (sessionId: string, terminalApp?: TerminalApp) => void
  onStop?: (sessionId: string) => void
  onComplete?: (sessionId: string) => void
  // Lazy loading - now uses combined /full endpoint (1 request instead of 3)
  getSessionFull?: (sessionId: string) => SessionFullData | undefined
  loadSessionFull?: (sessionId: string) => Promise<SessionFullData | null>
  isLoadingFull?: (sessionId: string) => boolean
  // Pagination
  pagination?: PaginationInfo | null
  onLoadMore?: () => Promise<void>
  loadingMore?: boolean
  hasActiveFilters?: boolean
  totalCount?: number
  globalMatchCount?: number
  initiallyExpandedSessionId?: string
  onInitialExpansionConsumed?: () => void
}

interface GroupedSessions {
  needs_input: Session[]
  stalled: Session[]
  interrupted: Session[]
  running: Session[]
  waiting: Session[]
  idle: Session[]
  lost: Session[]
  completed: Session[]
}

export const SessionList = memo(function SessionList({
  sessions,
  onRecover,
  onStop,
  onComplete,
  getSessionFull,
  loadSessionFull,
  isLoadingFull,
  pagination,
  onLoadMore,
  loadingMore,
  hasActiveFilters = false,
  totalCount = sessions.length,
  globalMatchCount = pagination?.total ?? sessions.length,
  initiallyExpandedSessionId,
  onInitialExpansionConsumed,
}: SessionListProps) {
  // Memoize grouped and sorted sessions
  const groupedSessions = useMemo(() => {
    const groups: GroupedSessions = {
      needs_input: [], stalled: [], interrupted: [],
      running: [],
      waiting: [],
      idle: [],
      lost: [],
      completed: [],
    }

    // Single pass grouping
    for (const session of sessions) {
      const status = session.status as SessionStatus
      if (groups[status]) {
        groups[status].push(session)
      }
    }

    // Sort each group by lastActiveAt (most recent first)
    const sortByTime = (a: Session, b: Session) => {
      const timeA = new Date(a.lastActiveAt).getTime()
      const timeB = new Date(b.lastActiveAt).getTime()
      return timeB - timeA
    }

    for (const status of STATUS_ORDER) {
      groups[status].sort(sortByTime)
    }

    return groups
  }, [sessions])

  if (sessions.length === 0) {
    const message = totalCount === 0
      ? 'No sessions found. Click Sync to scan for sessions.'
      : hasActiveFilters && globalMatchCount === 0
        ? 'No sessions match the current search or filters.'
        : 'No sessions are loaded for this page.'

    return (
      <div className={styles.empty}>
        {message}
      </div>
    )
  }

  return (
    <div className={styles.container}>
      {STATUS_ORDER.filter((status) => groupedSessions[status].length > 0).map((status) => (
        <SessionGroup
          key={status}
          title={status === 'waiting' ? 'Waiting for Input' : SESSION_STATUS_PRESENTATION[status].label}
          sessions={groupedSessions[status]}
          onRecover={onRecover}
          onStop={onStop}
          onComplete={onComplete}
          getSessionFull={getSessionFull}
          loadSessionFull={loadSessionFull}
          isLoadingFull={isLoadingFull}
          defaultCollapsed={status === 'completed'}
          initiallyExpandedSessionId={initiallyExpandedSessionId}
          onInitialExpansionConsumed={onInitialExpansionConsumed}
        />
      ))}

      {/* Load More button */}
      {pagination?.hasMore && onLoadMore && (
        <div className={styles.loadMore}>
          <Button
            variant="secondary"
            onClick={onLoadMore}
            disabled={loadingMore}
          >
            {loadingMore ? 'Loading...' : `Load More (${pagination.total - sessions.length} remaining)`}
          </Button>
        </div>
      )}
    </div>
  )
})

interface SessionGroupProps {
  title: string
  sessions: Session[]
  onRecover?: (sessionId: string, terminalApp?: TerminalApp) => void
  onStop?: (sessionId: string) => void
  onComplete?: (sessionId: string) => void
  getSessionFull?: (sessionId: string) => SessionFullData | undefined
  loadSessionFull?: (sessionId: string) => Promise<SessionFullData | null>
  isLoadingFull?: (sessionId: string) => boolean
  defaultCollapsed?: boolean
  initiallyExpandedSessionId?: string
  onInitialExpansionConsumed?: () => void
}

const SessionGroup = memo(function SessionGroup({
  title,
  sessions,
  onRecover,
  onStop,
  onComplete,
  getSessionFull,
  loadSessionFull,
  isLoadingFull,
  defaultCollapsed = false,
  initiallyExpandedSessionId,
  onInitialExpansionConsumed,
}: SessionGroupProps) {
  const [collapsed, setCollapsed] = useState(defaultCollapsed)
  const hasExpansionTarget = Boolean(
    initiallyExpandedSessionId &&
    sessions.some((session) => session.sessionId === initiallyExpandedSessionId)
  )

  useEffect(() => {
    if (hasExpansionTarget) setCollapsed(false)
  }, [hasExpansionTarget])

  useEffect(() => {
    if (hasExpansionTarget && !collapsed) {
      onInitialExpansionConsumed?.()
    }
  }, [collapsed, hasExpansionTarget, onInitialExpansionConsumed])

  const toggleCollapsed = useCallback(() => {
    setCollapsed(prev => !prev)
  }, [])

  return (
    <div className={styles.group}>
      <h3 className={styles.groupTitle} onClick={toggleCollapsed}>
        <span className={styles.collapseIcon}>{collapsed ? '▶' : '▼'}</span>
        {title} ({sessions.length})
      </h3>
      {!collapsed && (
        <div className={styles.list}>
          {sessions.map(session => (
            <SessionCard
              key={session.sessionId}
              session={session}
              onRecover={onRecover}
              onStop={onStop}
              onComplete={onComplete}
              getSessionFull={getSessionFull}
              loadSessionFull={loadSessionFull}
              isLoadingFull={isLoadingFull}
              defaultExpanded={session.sessionId === initiallyExpandedSessionId}
            />
          ))}
        </div>
      )}
    </div>
  )
})
