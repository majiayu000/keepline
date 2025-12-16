import { useState, useMemo, memo, useCallback } from 'react'
import type { Session, SessionFullData, PaginationInfo, TerminalApp } from '@/types'
import { SessionCard } from '@/components/SessionCard'
import { Button } from '@/components/Button'
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
}

type SessionStatus = 'running' | 'waiting' | 'idle' | 'lost' | 'completed'

interface GroupedSessions {
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
}: SessionListProps) {
  // Memoize grouped and sorted sessions
  const groupedSessions = useMemo(() => {
    const groups: GroupedSessions = {
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

    for (const key of Object.keys(groups) as SessionStatus[]) {
      groups[key].sort(sortByTime)
    }

    return groups
  }, [sessions])

  if (sessions.length === 0) {
    return (
      <div className={styles.empty}>
        No sessions found. Click Sync to scan for sessions.
      </div>
    )
  }

  return (
    <div className={styles.container}>
      {/* Priority: Running, Waiting, Idle */}
      {groupedSessions.running.length > 0 && (
        <SessionGroup
          title="Running"
          sessions={groupedSessions.running}
          onRecover={onRecover}
          onStop={onStop}
          onComplete={onComplete}
          getSessionFull={getSessionFull}
          loadSessionFull={loadSessionFull}
          isLoadingFull={isLoadingFull}
        />
      )}
      {groupedSessions.waiting.length > 0 && (
        <SessionGroup
          title="Waiting for Input"
          sessions={groupedSessions.waiting}
          onRecover={onRecover}
          onStop={onStop}
          onComplete={onComplete}
          getSessionFull={getSessionFull}
          loadSessionFull={loadSessionFull}
          isLoadingFull={isLoadingFull}
        />
      )}
      {groupedSessions.idle.length > 0 && (
        <SessionGroup
          title="Idle"
          sessions={groupedSessions.idle}
          onRecover={onRecover}
          onStop={onStop}
          onComplete={onComplete}
          getSessionFull={getSessionFull}
          loadSessionFull={loadSessionFull}
          isLoadingFull={isLoadingFull}
        />
      )}
      {/* Secondary: Lost, Completed */}
      {groupedSessions.lost.length > 0 && (
        <SessionGroup
          title="Lost"
          sessions={groupedSessions.lost}
          onRecover={onRecover}
          onStop={onStop}
          onComplete={onComplete}
          getSessionFull={getSessionFull}
          loadSessionFull={loadSessionFull}
          isLoadingFull={isLoadingFull}
        />
      )}
      {groupedSessions.completed.length > 0 && (
        <SessionGroup
          title="Completed"
          sessions={groupedSessions.completed}
          onRecover={onRecover}
          onStop={onStop}
          onComplete={onComplete}
          getSessionFull={getSessionFull}
          loadSessionFull={loadSessionFull}
          isLoadingFull={isLoadingFull}
          defaultCollapsed
        />
      )}

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
  defaultCollapsed = false
}: SessionGroupProps) {
  const [collapsed, setCollapsed] = useState(defaultCollapsed)

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
            />
          ))}
        </div>
      )}
    </div>
  )
})
