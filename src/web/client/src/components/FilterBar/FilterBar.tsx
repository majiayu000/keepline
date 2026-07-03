import { memo, useCallback } from 'react'
import type { RuntimeFilter, SessionStatus, SessionStats } from '@/types'
import { STATUS_LABELS, STATUS_COLORS, STATUS_ORDER } from '@/constants'
import styles from './FilterBar.module.css'

export type StatusFilter = SessionStatus | 'all'

interface FilterBarProps {
  activeFilters: Set<SessionStatus>
  onFilterChange: (filters: Set<SessionStatus>) => void
  runtimeFilter: RuntimeFilter
  onRuntimeFilterChange: (filter: RuntimeFilter) => void
  stats?: SessionStats | null
}

const RUNTIME_OPTIONS: Array<{ value: RuntimeFilter; label: string }> = [
  { value: 'all', label: 'All' },
  { value: 'claude-code', label: 'Claude Code' },
  { value: 'codex', label: 'Codex' },
]

export const FilterBar = memo(function FilterBar({
  activeFilters,
  onFilterChange,
  runtimeFilter,
  onRuntimeFilterChange,
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
    onRuntimeFilterChange('all')
  }, [onFilterChange, onRuntimeFilterChange])

  const getCount = (status: SessionStatus): number => {
    if (!stats) return 0
    return stats[status] || 0
  }

  const hasActiveFilters = activeFilters.size > 0 || runtimeFilter !== 'all'

  return (
    <div className={styles.container}>
      <span className={styles.label}>Runtime:</span>
      <div className={styles.filters}>
        {RUNTIME_OPTIONS.map((option) => (
          <button
            key={option.value}
            className={`${styles.filterChip} ${runtimeFilter === option.value ? styles.active : ''}`}
            onClick={() => onRuntimeFilterChange(option.value)}
            aria-pressed={runtimeFilter === option.value}
          >
            {option.label}
          </button>
        ))}
      </div>
      <span className={styles.label}>Status:</span>
      <div className={styles.filters}>
        {STATUS_ORDER.map((status) => (
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
