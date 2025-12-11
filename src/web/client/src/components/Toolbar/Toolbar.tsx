import { memo } from 'react'
import { SearchBar } from '@/components/SearchBar'
import { FilterBar } from '@/components/FilterBar'
import type { SessionStatus, SessionStats } from '@/types'
import styles from './Toolbar.module.css'

interface ToolbarProps {
  searchQuery: string
  onSearchChange: (query: string) => void
  statusFilters: Set<SessionStatus>
  onFilterChange: (filters: Set<SessionStatus>) => void
  stats?: SessionStats | null
  totalCount: number
  filteredCount: number
}

export const Toolbar = memo(function Toolbar({
  searchQuery,
  onSearchChange,
  statusFilters,
  onFilterChange,
  stats,
  totalCount,
  filteredCount,
}: ToolbarProps) {
  const hasFilters = searchQuery.length > 0 || statusFilters.size > 0
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
          stats={stats}
        />
      </div>
    </div>
  )
})
