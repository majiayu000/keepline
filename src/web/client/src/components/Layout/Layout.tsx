import { ReactNode, memo } from 'react'
import { Header } from '@/components/Header'
import { StatsBar } from '@/components/StatsBar'
import { Toolbar } from '@/components/Toolbar'
import type { Session, SessionStats, SessionStatus } from '@/types'
import styles from './Layout.module.css'

interface LayoutProps {
  children: ReactNode
  stats: SessionStats | null
  loading?: boolean
  onSync: () => void
  syncing?: boolean
  // Search & Filter props
  searchQuery?: string
  onSearchChange?: (query: string) => void
  statusFilters?: Set<SessionStatus>
  onFilterChange?: (filters: Set<SessionStatus>) => void
  totalCount?: number
  filteredCount?: number
  // Export props
  sessions?: Session[]
}

export const Layout = memo(function Layout({
  children,
  stats,
  loading,
  onSync,
  syncing,
  searchQuery = '',
  onSearchChange,
  statusFilters = new Set(),
  onFilterChange,
  totalCount = 0,
  filteredCount = 0,
  sessions = [],
}: LayoutProps) {
  const showToolbar = onSearchChange && onFilterChange

  return (
    <div className={styles.layout}>
      <Header onSync={onSync} syncing={syncing} sessions={sessions} />
      <StatsBar stats={stats} loading={loading} />
      {showToolbar && (
        <Toolbar
          searchQuery={searchQuery}
          onSearchChange={onSearchChange}
          statusFilters={statusFilters}
          onFilterChange={onFilterChange}
          stats={stats}
          totalCount={totalCount}
          filteredCount={filteredCount}
        />
      )}
      <main className={styles.main}>{children}</main>
    </div>
  )
})

// Export styles for use in App.tsx
export { styles as layoutStyles }
