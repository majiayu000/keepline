import { memo, useCallback } from 'react'
import type { SessionStatus, SessionStats } from '@/types'
import { STATUS_LABELS, STATUS_COLORS } from '@/constants'
import styles from './FilterBar.module.css'

export type StatusFilter = SessionStatus | 'all'

interface FilterBarProps {
  activeFilters: Set<SessionStatus>
  onFilterChange: (filters: Set<SessionStatus>) => void
  stats?: SessionStats | null
}

const ALL_STATUSES: SessionStatus[] = ['running', 'waiting', 'idle', 'lost', 'completed']

export const FilterBar = memo(function FilterBar({
  activeFilters,
  onFilterChange,
  stats,
}: FilterBarProps) {
  const toggleFilter = useCallback((status: SessionStatus) => {
    const newFilters = new Set(activeFilters)
    if (newFilters.has(status)) {
      newFilters.delete(status)
    } else {
      newFilters.add(status)
    }
    onFilterChange(newFilters)
  }, [activeFilters, onFilterChange])

  const clearFilters = useCallback(() => {
    onFilterChange(new Set())
  }, [onFilterChange])

  const getCount = (status: SessionStatus): number => {
    if (!stats) return 0
    return stats[status] || 0
  }

  const hasActiveFilters = activeFilters.size > 0

  return (
    <div className={styles.container}>
      <span className={styles.label}>Filter:</span>
      <div className={styles.filters}>
        {ALL_STATUSES.map((status) => (
          <button
            key={status}
            className={`${styles.filterChip} ${activeFilters.has(status) ? styles.active : ''}`}
            onClick={() => toggleFilter(status)}
            aria-pressed={activeFilters.has(status)}
          >
            <span
              className={styles.statusDot}
              style={{ backgroundColor: STATUS_COLORS[status] }}
            />
            {STATUS_LABELS[status]}
            <span className={styles.count}>({getCount(status)})</span>
          </button>
        ))}
      </div>
      {hasActiveFilters && (
        <button className={styles.clearAll} onClick={clearFilters}>
          Clear
        </button>
      )}
    </div>
  )
})
