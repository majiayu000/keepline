import { useState, useCallback, useMemo, useEffect } from 'react'
import { useMemory } from '@/hooks'
import { MemoryCard } from '@/components/MemoryCard'
import { Button } from '@/components/Button'
import type { MemoryContext } from '@/types'
import styles from './MemoryPanel.module.css'

export function MemoryPanel() {
  const {
    memories,
    loading,
    error,
    refresh,
    loadContext,
    deleteMemory,
  } = useMemory()

  const [searchQuery, setSearchQuery] = useState('')
  const [contextModal, setContextModal] = useState<{
    sessionId: string
    context: MemoryContext | null
    loading: boolean
  } | null>(null)

  // Filter memories by search query
  const filteredMemories = useMemo(() => {
    if (!searchQuery.trim()) return memories
    const query = searchQuery.toLowerCase()
    return memories.filter(m =>
      m.directory.toLowerCase().includes(query) ||
      m.lastProgress.toLowerCase().includes(query) ||
      m.notes.toLowerCase().includes(query)
    )
  }, [memories, searchQuery])

  // Calculate stats
  const stats = useMemo(() => {
    const totalPending = memories.reduce((sum, m) => sum + m.pendingTasks.length, 0)
    const totalCompleted = memories.reduce((sum, m) => sum + m.completedTasks.length, 0)
    const totalIterations = memories.reduce((sum, m) => sum + m.iterationCount, 0)
    return {
      totalMemories: memories.length,
      totalPending,
      totalCompleted,
      totalIterations,
    }
  }, [memories])

  // Handle view context
  const handleViewContext = useCallback(async (sessionId: string) => {
    setContextModal({ sessionId, context: null, loading: true })
    const ctx = await loadContext(sessionId)
    setContextModal(prev => prev ? { ...prev, context: ctx, loading: false } : null)
  }, [loadContext])

  // Handle delete
  const handleDelete = useCallback(async (sessionId: string) => {
    if (window.confirm('Delete this memory? This cannot be undone.')) {
      await deleteMemory(sessionId)
    }
  }, [deleteMemory])

  // Handle copy context
  const handleCopyContext = useCallback(() => {
    if (contextModal?.context) {
      navigator.clipboard.writeText(contextModal.context.context)
    }
  }, [contextModal])

  // Close modal on escape
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && contextModal) {
        setContextModal(null)
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [contextModal])

  if (loading) {
    return (
      <div className={styles.panel}>
        <div className={styles.loading}>
          <span className={styles.loadingSpinner}>@</span>
          Loading memories...
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
      <div className={styles.statsBar}>
        <div className={styles.statCard}>
          <span className={styles.statValue}>{stats.totalMemories}</span>
          <span className={styles.statLabel}>Memories</span>
        </div>
        <div className={`${styles.statCard} ${styles.pending}`}>
          <span className={styles.statValue}>{stats.totalPending}</span>
          <span className={styles.statLabel}>Pending</span>
        </div>
        <div className={`${styles.statCard} ${styles.completed}`}>
          <span className={styles.statValue}>{stats.totalCompleted}</span>
          <span className={styles.statLabel}>Completed</span>
        </div>
        <div className={`${styles.statCard} ${styles.iterations}`}>
          <span className={styles.statValue}>{stats.totalIterations}</span>
          <span className={styles.statLabel}>Iterations</span>
        </div>
      </div>

      {/* Toolbar */}
      <div className={styles.toolbar}>
        <div className={styles.searchBox}>
          <span className={styles.searchIcon}>/</span>
          <input
            type="text"
            className={styles.searchInput}
            placeholder="Search memories..."
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

      {/* Memory List or Empty State */}
      {filteredMemories.length === 0 ? (
        <div className={styles.emptyState}>
          <div className={styles.emptyIcon}>[_]</div>
          <div className={styles.emptyTitle}>No Memories Found</div>
          <div className={styles.emptyText}>
            {searchQuery
              ? 'No memories match your search. Try a different query.'
              : 'Session memories will appear here once agent sessions store their context for recovery.'}
          </div>
        </div>
      ) : (
        <div className={styles.memoryList}>
          {filteredMemories.map((memory) => (
            <MemoryCard
              key={memory.sessionId}
              memory={memory}
              onDelete={handleDelete}
              onViewContext={handleViewContext}
            />
          ))}
        </div>
      )}

      {/* Context Modal */}
      {contextModal && (
        <div
          className={styles.contextModal}
          onClick={() => setContextModal(null)}
        >
          <div
            className={styles.contextContent}
            onClick={(e) => e.stopPropagation()}
          >
            <div className={styles.contextHeader}>
              <span className={styles.contextTitle}>
                Recovery Context
                {contextModal.context && ` (Iteration ${contextModal.context.iterationCount})`}
              </span>
              <button
                className={styles.closeButton}
                onClick={() => setContextModal(null)}
              >
                x
              </button>
            </div>
            <div className={styles.contextBody}>
              {contextModal.loading ? (
                <div className={styles.loading}>
                  <span className={styles.loadingSpinner}>@</span>
                  Loading context...
                </div>
              ) : contextModal.context ? (
                <pre className={styles.contextText}>
                  {contextModal.context.context}
                </pre>
              ) : (
                <div className={styles.error}>Failed to load context</div>
              )}
            </div>
            {contextModal.context && (
              <div className={styles.contextActions}>
                <Button
                  variant="primary"
                  size="sm"
                  onClick={handleCopyContext}
                >
                  Copy to Clipboard
                </Button>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => setContextModal(null)}
                >
                  Close
                </Button>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
