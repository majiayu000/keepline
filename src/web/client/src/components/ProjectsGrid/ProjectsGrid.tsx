import { memo } from 'react'
import type { ProjectInfo } from '@/types/project'
import { ProjectCard } from '@/components/ProjectCard'
import styles from './ProjectsGrid.module.css'

export interface ProjectsGridProps {
  projects: ProjectInfo[]
  onProjectClick?: (projectPath: string) => void
}

export const ProjectsGrid = memo(function ProjectsGrid({
  projects,
  onProjectClick,
}: ProjectsGridProps) {
  if (projects.length === 0) {
    return (
      <div className={styles.empty}>
        <span className={styles.emptyIcon}>#</span>
        <span className={styles.emptyText}>No projects found</span>
        <span className={styles.emptyHint}>
          Start a Claude Code session to see projects here
        </span>
      </div>
    )
  }

  return (
    <div className={styles.grid}>
      {projects.map((project) => (
        <ProjectCard
          key={project.path}
          project={project}
          onClick={onProjectClick}
        />
      ))}
    </div>
  )
})
