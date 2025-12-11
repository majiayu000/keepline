import { memo, useCallback } from 'react'
import type { ProjectInfo } from '@/types/project'
import { getProjectActivityStatus } from '@/types/project'
import { formatRelativeTime, formatCost } from '@/utils/format'
import styles from './ProjectCard.module.css'

export interface ProjectCardProps {
  project: ProjectInfo
  onClick?: (projectPath: string) => void
}

/** Status indicator colors */
const STATUS_COLORS = {
  running: 'var(--success)',
  waiting: 'var(--warning)',
  idle: 'var(--info)',
  lost: 'var(--danger)',
  completed: 'var(--text-dim)',
}

export const ProjectCard = memo(function ProjectCard({
  project,
  onClick,
}: ProjectCardProps) {
  const { name, path, stats, currentTask, lastActiveAt, totalUsage } = project

  const handleClick = useCallback(() => {
    onClick?.(path)
  }, [onClick, path])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault()
        onClick?.(path)
      }
    },
    [onClick, path]
  )

  const activityStatus = getProjectActivityStatus(stats)

  // Truncate current task to ~40 chars
  const truncatedTask = currentTask
    ? currentTask.length > 40
      ? currentTask.slice(0, 40) + '...'
      : currentTask
    : undefined

  return (
    <div
      className={`${styles.card} ${styles[activityStatus]}`}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      role="button"
      tabIndex={0}
      aria-label={`Project ${name}, ${stats.total} sessions`}
    >
      <div className={styles.header}>
        <h3 className={styles.projectName}>{name}</h3>
        {totalUsage && totalUsage.totalCost > 0 && (
          <span className={styles.cost}>{formatCost(totalUsage.totalCost)}</span>
        )}
      </div>

      <div className={styles.statusRow}>
        {stats.running > 0 && (
          <span className={styles.statusBadge} style={{ color: STATUS_COLORS.running }}>
            <span className={styles.statusDot} style={{ backgroundColor: STATUS_COLORS.running }} />
            {stats.running}
          </span>
        )}
        {stats.waiting > 0 && (
          <span className={styles.statusBadge} style={{ color: STATUS_COLORS.waiting }}>
            <span className={styles.statusDot} style={{ backgroundColor: STATUS_COLORS.waiting }} />
            {stats.waiting}
          </span>
        )}
        {stats.idle > 0 && (
          <span className={styles.statusBadge} style={{ color: STATUS_COLORS.idle }}>
            <span className={styles.statusDot} style={{ backgroundColor: STATUS_COLORS.idle }} />
            {stats.idle}
          </span>
        )}
        {stats.lost > 0 && (
          <span className={styles.statusBadge} style={{ color: STATUS_COLORS.lost }}>
            <span className={styles.statusDot} style={{ backgroundColor: STATUS_COLORS.lost }} />
            {stats.lost}
          </span>
        )}
        {stats.completed > 0 && (
          <span className={styles.statusBadge} style={{ color: STATUS_COLORS.completed }}>
            <span className={styles.statusDot} style={{ backgroundColor: STATUS_COLORS.completed }} />
            {stats.completed}
          </span>
        )}
      </div>

      {truncatedTask && (
        <div className={styles.currentTask} title={currentTask}>
          {truncatedTask}
        </div>
      )}

      {!truncatedTask && (
        <div className={styles.noTask}>(no active task)</div>
      )}

      <div className={styles.footer}>
        <span className={styles.lastActive}>
          {formatRelativeTime(lastActiveAt)}
        </span>
        <span className={styles.sessionCount}>
          {stats.total} session{stats.total !== 1 ? 's' : ''}
        </span>
      </div>
    </div>
  )
})
