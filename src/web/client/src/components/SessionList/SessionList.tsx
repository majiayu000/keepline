import type { Session } from '@/types'
import { SessionCard } from '@/components/SessionCard'
import styles from './SessionList.module.css'

interface SessionListProps {
  sessions: Session[]
  onRecover?: (sessionId: string) => void
  onStop?: (sessionId: string) => void
  onComplete?: (sessionId: string) => void
}

export function SessionList({ sessions, onRecover, onStop, onComplete }: SessionListProps) {
  if (sessions.length === 0) {
    return (
      <div className={styles.empty}>
        No sessions found. Click Sync to scan for sessions.
      </div>
    )
  }

  // Group sessions by status
  const running = sessions.filter(s => s.status === 'running')
  const waiting = sessions.filter(s => s.status === 'waiting')
  const idle = sessions.filter(s => s.status === 'idle')
  const lost = sessions.filter(s => s.status === 'lost')
  const completed = sessions.filter(s => s.status === 'completed')

  return (
    <div className={styles.container}>
      {running.length > 0 && (
        <SessionGroup
          title="Running"
          sessions={running}
          onRecover={onRecover}
          onStop={onStop}
          onComplete={onComplete}
        />
      )}
      {waiting.length > 0 && (
        <SessionGroup
          title="Waiting for Input"
          sessions={waiting}
          onRecover={onRecover}
          onStop={onStop}
          onComplete={onComplete}
        />
      )}
      {lost.length > 0 && (
        <SessionGroup
          title="Lost"
          sessions={lost}
          onRecover={onRecover}
          onStop={onStop}
          onComplete={onComplete}
        />
      )}
      {idle.length > 0 && (
        <SessionGroup
          title="Idle"
          sessions={idle}
          onRecover={onRecover}
          onStop={onStop}
          onComplete={onComplete}
        />
      )}
      {completed.length > 0 && (
        <SessionGroup
          title="Completed"
          sessions={completed}
          onRecover={onRecover}
          onStop={onStop}
          onComplete={onComplete}
          collapsed
        />
      )}
    </div>
  )
}

interface SessionGroupProps {
  title: string
  sessions: Session[]
  onRecover?: (sessionId: string) => void
  onStop?: (sessionId: string) => void
  onComplete?: (sessionId: string) => void
  collapsed?: boolean
}

function SessionGroup({ title, sessions, onRecover, onStop, onComplete }: SessionGroupProps) {
  return (
    <div className={styles.group}>
      <h3 className={styles.groupTitle}>
        {title} ({sessions.length})
      </h3>
      <div className={styles.list}>
        {sessions.map(session => (
          <SessionCard
            key={session.sessionId}
            session={session}
            onRecover={onRecover}
            onStop={onStop}
            onComplete={onComplete}
          />
        ))}
      </div>
    </div>
  )
}
