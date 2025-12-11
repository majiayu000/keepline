import { memo } from 'react'
import type { ProjectOverviewStats } from '@/types/project'
import styles from './ProjectStatsBar.module.css'

export interface ProjectStatsBarProps {
  stats: ProjectOverviewStats
}

export const ProjectStatsBar = memo(function ProjectStatsBar({
  stats,
}: ProjectStatsBarProps) {
  return (
    <div className={styles.container}>
      <div className={styles.stat}>
        <span className={styles.emoji}>📊</span>
        <span className={`${styles.value} ${styles.default}`}>{stats.total}</span>
        <span className={styles.label}>Projects</span>
      </div>

      <div className={styles.divider}>│</div>

      <div className={styles.stat}>
        <span className={styles.emoji}>🟢</span>
        <span className={`${styles.value} ${styles.success}`}>{stats.active}</span>
        <span className={styles.label}>Active</span>
      </div>

      <div className={styles.divider}>│</div>

      <div className={styles.stat}>
        <span className={styles.emoji}>💤</span>
        <span className={`${styles.value} ${styles.info}`}>{stats.idle}</span>
        <span className={styles.label}>Idle</span>
      </div>
    </div>
  )
})
