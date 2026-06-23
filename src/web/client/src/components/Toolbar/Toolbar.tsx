import { memo } from 'react'
import { SearchBar } from '@/components/SearchBar'
import { FilterBar } from '@/components/FilterBar'
import type { RuntimeFilter, SessionStatus, SessionStats } from '@/types'
import styles from './Toolbar.module.css'

interface ToolbarProps {
  searchQuery: string
  onSearchChange: (query: string) => void
  statusFilters: Set<SessionStatus>
  onFilterChange: (filters: Set<SessionStatus>) => void
  runtimeFilter: RuntimeFilter
  onRuntimeFilterChange: (filter: RuntimeFilter) => void
  stats?: SessionStats | null
  totalCount: number
  filteredCount: number
}

export const Toolbar = memo(function Toolbar({
  searchQuery,
  onSearchChange,
  statusFilters,
  onFilterChange,
  runtimeFilter,
  onRuntimeFilterChange,
  stats,
  totalCount,
  filteredCount,
}: ToolbarProps) {
  const hasFilters = searchQuery.length > 0 || statusFilters.size > 0 || runtimeFilter !== 'all'
  const showResultsInfo = hasFilters && totalCount !== filteredCount

  return (
    <div className={styles.toolbar}>
      <div className={styles.row}>
        <div className={styles.searchWrapper}>
          <SearchBar
            value={searchQuery}
            onChange={onSearchChange}
            placeholder="Search by title, directory, prompt..."
          />
        </div>
        {showResultsInfo && (
          <div className={styles.resultsInfo}>
            <span className={styles.resultsCount}>{filteredCount}</span>
            <span>of {totalCount} sessions</span>
            {statusFilters.size > 0 && (
              <span className={styles.filterInfo}>
                {statusFilters.size} filter{statusFilters.size > 1 ? 's' : ''}
              </span>
            )}
          </div>
        )}
      </div>
      <div className={styles.row}>
        <FilterBar
          activeFilters={statusFilters}
          onFilterChange={onFilterChange}
          runtimeFilter={runtimeFilter}
          onRuntimeFilterChange={onRuntimeFilterChange}
          stats={stats}
        />
      </div>
    </div>
  )
})
