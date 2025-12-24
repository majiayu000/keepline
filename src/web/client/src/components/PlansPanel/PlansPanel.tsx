import { useState, useMemo } from 'react'
import { usePlans } from '@/hooks'
import { PlanCard } from '@/components/PlanCard'
import styles from './PlansPanel.module.css'

export function PlansPanel() {
  const {
    plans,
    stats,
    loading,
    error,
    refresh,
    loadPlan,
    isLoadingPlan,
  } = usePlans()

  const [searchQuery, setSearchQuery] = useState('')

  // Filter plans by search query
  const filteredPlans = useMemo(() => {
    if (!searchQuery.trim()) return plans
    const query = searchQuery.toLowerCase()
    return plans.filter(p =>
      p.title.toLowerCase().includes(query) ||
      p.id.toLowerCase().includes(query)
    )
  }, [plans, searchQuery])

  if (loading) {
    return (
      <div className={styles.panel}>
        <div className={styles.loading}>
          <span className={styles.loadingSpinner}>%</span>
          Loading plans...
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className={styles.panel}>
        <div className={styles.error}>Error: {error}</div>
      </div>
    )
  }

  return (
    <div className={styles.panel}>
      {/* Stats Bar */}
      {stats && (
        <div className={styles.statsBar}>
          <div className={`${styles.statCard} ${styles.plans}`}>
            <span className={styles.statValue}>{stats.totalPlans}</span>
            <span className={styles.statLabel}>Plans</span>
          </div>
          <div className={`${styles.statCard} ${styles.pending}`}>
            <span className={styles.statValue}>{stats.totalTasks - stats.completedTasks}</span>
            <span className={styles.statLabel}>Pending</span>
          </div>
          <div className={`${styles.statCard} ${styles.completed}`}>
            <span className={styles.statValue}>{stats.completedTasks}</span>
            <span className={styles.statLabel}>Completed</span>
          </div>
          <div className={`${styles.statCard} ${styles.progress}`}>
            <span className={styles.statValue}>{stats.overallCompletion}%</span>
            <span className={styles.statLabel}>Overall</span>
          </div>
        </div>
      )}

      {/* Toolbar */}
      <div className={styles.toolbar}>
        <div className={styles.searchBox}>
          <span className={styles.searchIcon}>/</span>
          <input
            type="text"
            className={styles.searchInput}
            placeholder="Search plans..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
        </div>
        <button
          className={`${styles.refreshButton} ${loading ? styles.loading : ''}`}
          onClick={refresh}
          title="Refresh"
        >
          @
        </button>
      </div>

      {/* Plan List or Empty State */}
      {filteredPlans.length === 0 ? (
        <div className={styles.emptyState}>
          <div className={styles.emptyIcon}>[%]</div>
          <div className={styles.emptyTitle}>No Plans Found</div>
          <div className={styles.emptyText}>
            {searchQuery
              ? 'No plans match your search. Try a different query.'
              : 'Claude Code plans will appear here. Plans are stored in ~/.claude/plans/'}
          </div>
        </div>
      ) : (
        <div className={styles.planList}>
          {filteredPlans.map((plan) => (
            <PlanCard
              key={plan.id}
              plan={plan}
              loadPlan={loadPlan}
              isLoading={isLoadingPlan(plan.id)}
            />
          ))}
        </div>
      )}
    </div>
  )
}
