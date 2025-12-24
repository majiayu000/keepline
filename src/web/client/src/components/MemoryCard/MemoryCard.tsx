import { memo, useCallback } from 'react'
import type { SessionMemory } from '@/types'
import { Button } from '@/components/Button'
import { formatRelativeTime, formatPath } from '@/utils/format'
import { useToggle } from '@/hooks'
import styles from './MemoryCard.module.css'

interface MemoryCardProps {
  memory: SessionMemory
  onDelete?: (sessionId: string) => void
  onViewContext?: (sessionId: string) => void
}

export const MemoryCard = memo(function MemoryCard({
  memory,
  onDelete,
  onViewContext,
}: MemoryCardProps) {
  const [expanded, toggle] = useToggle(false)

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      toggle()
    }
  }, [toggle])

  const hasPendingTasks = memory.pendingTasks.length > 0
  const hasCompletedTasks = memory.completedTasks.length > 0
  const hasHandoff = memory.handoffNotes || memory.handoffPriority.length > 0
  const hasIssues = memory.knownIssues.length > 0
  const hasDecisions = memory.decisions.length > 0
  const hasNotes = memory.notes.trim().length > 0

  return (
    <article className={`${styles.card} ${expanded ? styles.expanded : ''}`}>
      <div
        className={styles.header}
        onClick={toggle}
        onKeyDown={handleKeyDown}
        role="button"
        tabIndex={0}
        aria-expanded={expanded}
      >
        <div className={styles.headerLeft}>
          <div className={styles.directory} title={memory.directory}>
            {formatPath(memory.directory)}
          </div>
          {memory.lastProgress && (
            <div className={styles.progress}>{memory.lastProgress}</div>
          )}
          <div className={styles.meta}>
            <span className={styles.stat}>
              <span className={styles.statIcon}>*</span>
              <span className={styles.statValue}>{memory.pendingTasks.length}</span>
              <span>pending</span>
            </span>
            <span className={styles.stat}>
              <span className={styles.statIcon}>+</span>
              <span className={styles.statValue}>{memory.completedTasks.length}</span>
              <span>done</span>
            </span>
            <span className={styles.timestamp}>
              {formatRelativeTime(memory.updatedAt)}
            </span>
          </div>
        </div>
        <div className={styles.headerRight}>
          <div className={styles.iteration} title="Iteration count">
            {memory.iterationCount}
          </div>
          <span className={styles.expandIcon} aria-hidden="true">
            &gt;
          </span>
        </div>
      </div>

      {expanded && (
        <div className={styles.details}>
          {/* Pending Tasks */}
          {hasPendingTasks && (
            <section className={styles.section}>
              <h4 className={styles.sectionTitle}>
                <span className={styles.sectionIcon}>*</span>
                Pending Tasks ({memory.pendingTasks.length})
              </h4>
              <ul className={styles.taskList}>
                {memory.pendingTasks.map((task, i) => (
                  <li key={i} className={`${styles.taskItem} ${styles.taskPending}`}>
                    <span className={styles.taskIcon}>[ ]</span>
                    <span className={styles.taskText}>{task}</span>
                  </li>
                ))}
              </ul>
            </section>
          )}

          {/* Completed Tasks */}
          {hasCompletedTasks && (
            <section className={styles.section}>
              <h4 className={styles.sectionTitle}>
                <span className={styles.sectionIcon}>+</span>
                Completed ({memory.completedTasks.length})
              </h4>
              <ul className={styles.taskList}>
                {memory.completedTasks.slice(0, 5).map((task, i) => (
                  <li key={i} className={`${styles.taskItem} ${styles.taskCompleted}`}>
                    <span className={styles.taskIcon}>[x]</span>
                    <span className={styles.taskText}>{task}</span>
                  </li>
                ))}
                {memory.completedTasks.length > 5 && (
                  <li className={styles.taskItem}>
                    <span className={styles.empty}>
                      +{memory.completedTasks.length - 5} more...
                    </span>
                  </li>
                )}
              </ul>
            </section>
          )}

          {/* Known Issues */}
          {hasIssues && (
            <section className={styles.section}>
              <h4 className={styles.sectionTitle}>
                <span className={styles.sectionIcon}>!</span>
                Known Issues
              </h4>
              <div className={styles.tagList}>
                {memory.knownIssues.map((issue, i) => (
                  <span key={i} className={`${styles.tag} ${styles.issueTag}`}>
                    {issue}
                  </span>
                ))}
              </div>
            </section>
          )}

          {/* Decisions */}
          {hasDecisions && (
            <section className={styles.section}>
              <h4 className={styles.sectionTitle}>
                <span className={styles.sectionIcon}>@</span>
                Decisions Made
              </h4>
              <div className={styles.tagList}>
                {memory.decisions.map((decision, i) => (
                  <span key={i} className={`${styles.tag} ${styles.decisionTag}`}>
                    {decision}
                  </span>
                ))}
              </div>
            </section>
          )}

          {/* Notes */}
          {hasNotes && (
            <section className={styles.section}>
              <h4 className={styles.sectionTitle}>
                <span className={styles.sectionIcon}>#</span>
                Notes
              </h4>
              <div className={styles.notesBox}>{memory.notes}</div>
            </section>
          )}

          {/* Handoff Section */}
          {hasHandoff && (
            <div className={styles.handoffSection}>
              <h4 className={styles.sectionTitle}>
                <span className={styles.sectionIcon}>&gt;&gt;</span>
                Handoff for Next Iteration
              </h4>
              {memory.handoffNotes && (
                <div className={styles.notesBox}>{memory.handoffNotes}</div>
              )}
              {memory.handoffPriority.length > 0 && (
                <div className={styles.priorityList}>
                  {memory.handoffPriority.map((priority, i) => (
                    <span key={i} className={styles.priorityItem}>
                      <span className={styles.priorityNumber}>{i + 1}</span>
                      {priority}
                    </span>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Actions */}
          <div className={styles.actions}>
            {onViewContext && (
              <Button
                variant="primary"
                size="sm"
                onClick={() => onViewContext(memory.sessionId)}
              >
                View Context
              </Button>
            )}
            {onDelete && (
              <Button
                variant="danger"
                size="sm"
                onClick={() => onDelete(memory.sessionId)}
              >
                Delete
              </Button>
            )}
          </div>
        </div>
      )}
    </article>
  )
})
