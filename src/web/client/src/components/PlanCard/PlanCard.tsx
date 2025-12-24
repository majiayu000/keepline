import { memo, useCallback, useEffect } from 'react'
import type { Plan } from '@/types'
import { formatRelativeTime } from '@/utils/format'
import { useToggle } from '@/hooks'
import styles from './PlanCard.module.css'

interface PlanCardProps {
  plan: Plan
  loadPlan?: (id: string) => Promise<Plan | null>
  isLoading?: boolean
}

export const PlanCard = memo(function PlanCard({
  plan,
  loadPlan,
  isLoading,
}: PlanCardProps) {
  const [expanded, toggle] = useToggle(false)

  // Load full plan content when expanded
  useEffect(() => {
    if (expanded && loadPlan && !plan.content) {
      loadPlan(plan.id)
    }
  }, [expanded, loadPlan, plan.id, plan.content])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      toggle()
    }
  }, [toggle])

  const isComplete = plan.stats.completionPercent === 100

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
          <div className={styles.title}>{plan.title}</div>
          <div className={styles.meta}>
            <span>{plan.stats.phaseCount} phases</span>
            <span>|</span>
            <span>{plan.stats.totalTasks} tasks</span>
            <span>|</span>
            <span className={styles.timestamp}>
              {formatRelativeTime(plan.modifiedAt)}
            </span>
          </div>
        </div>
        <div className={styles.headerRight}>
          <div className={styles.progressContainer}>
            <div className={styles.progressBar}>
              <div
                className={styles.progressFill}
                style={{ width: `${plan.stats.completionPercent}%` }}
              />
            </div>
            <span className={styles.progressText}>
              {plan.stats.completionPercent}%
            </span>
          </div>
          <div className={`${styles.completionBadge} ${isComplete ? styles.complete : ''}`}>
            {plan.stats.completedTasks}/{plan.stats.totalTasks}
          </div>
          <span className={styles.expandIcon} aria-hidden="true">
            &gt;
          </span>
        </div>
      </div>

      {expanded && (
        <div className={styles.details}>
          {isLoading && (
            <div className={styles.loading}>Loading plan details...</div>
          )}

          {/* Phases */}
          {plan.phases.length > 0 && (
            <div className={styles.phases}>
              {plan.phases.map((phase, i) => {
                const phasePercent = phase.totalCount > 0
                  ? Math.round((phase.completedCount / phase.totalCount) * 100)
                  : 0
                return (
                  <div key={i} className={styles.phase}>
                    <span className={styles.phaseName}>{phase.title}</span>
                    <div className={styles.phaseProgress}>
                      <div className={styles.phaseBar}>
                        <div
                          className={styles.phaseFill}
                          style={{ width: `${phasePercent}%` }}
                        />
                      </div>
                      <span className={styles.phaseCount}>
                        {phase.completedCount}/{phase.totalCount}
                      </span>
                    </div>
                  </div>
                )
              })}
            </div>
          )}

          {/* Tasks (show pending first, then completed) */}
          {plan.tasks && plan.tasks.length > 0 && (
            <ul className={styles.taskList}>
              {plan.tasks
                .filter(t => !t.completed)
                .slice(0, 10)
                .map((task, i) => (
                  <li key={`pending-${i}`} className={`${styles.taskItem} ${styles.taskPending}`}>
                    <span className={styles.taskIcon}>[ ]</span>
                    <span className={styles.taskText}>{task.text}</span>
                    {task.phase && (
                      <span className={styles.taskPhase}>{task.phase}</span>
                    )}
                  </li>
                ))}
              {plan.tasks
                .filter(t => t.completed)
                .slice(-5)
                .map((task, i) => (
                  <li key={`done-${i}`} className={`${styles.taskItem} ${styles.taskCompleted}`}>
                    <span className={styles.taskIcon}>[x]</span>
                    <span className={styles.taskText}>{task.text}</span>
                    {task.phase && (
                      <span className={styles.taskPhase}>{task.phase}</span>
                    )}
                  </li>
                ))}
            </ul>
          )}

          {/* Content Preview */}
          {plan.content && (
            <div className={styles.contentSection}>
              <h4 className={styles.sectionTitle}>Plan Content</h4>
              <pre className={styles.contentPreview}>
                {plan.content.slice(0, 2000)}
                {plan.content.length > 2000 && '...'}
              </pre>
            </div>
          )}
        </div>
      )}
    </article>
  )
})
