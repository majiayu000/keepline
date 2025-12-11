import { useState, useEffect, useCallback, memo, useMemo } from 'react'
import type { ToolCallInfo, ToolCallsData } from '@/types'
import { fetchToolCalls } from '@/services/api'
import { Spinner } from '@/components/Spinner'
import { Button } from '@/components/Button'
import { formatTime, formatInput } from '@/utils/format'
import { getToolColor } from '@/constants'
import styles from './ToolCallList.module.css'

/** Number of items to show per page */
const PAGE_SIZE = 20

interface ToolCallListProps {
  sessionId: string
  toolCount?: number
  // Cached data from parent (from /full endpoint) - avoids separate API call
  cachedData?: ToolCallsData
}

export const ToolCallList = memo(function ToolCallList({ sessionId, cachedData }: ToolCallListProps) {
  const [toolCalls, setToolCalls] = useState<ToolCallInfo[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [loaded, setLoaded] = useState(false)
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null)
  const [visibleCount, setVisibleCount] = useState(PAGE_SIZE)

  // Use cached data if available, otherwise fetch
  useEffect(() => {
    // If we have cached data from parent, use it directly (no API call needed)
    if (cachedData) {
      setToolCalls(cachedData.toolCalls)
      setLoaded(true)
      return
    }

    // Fallback: fetch if no cached data (backwards compatibility)
    async function loadToolCalls() {
      if (loaded) return
      setLoading(true)
      setError(null)
      try {
        const response = await fetchToolCalls(sessionId)
        if (response.success && response.data) {
          setToolCalls(response.data.toolCalls)
          setLoaded(true)
        } else {
          setError(response.error || 'Failed to load tool calls')
        }
      } catch {
        setError('Failed to load tool calls')
      } finally {
        setLoading(false)
      }
    }
    loadToolCalls()
  }, [sessionId, loaded, cachedData])

  const handleToggle = useCallback((index: number) => {
    setExpandedIndex(prev => prev === index ? null : index)
  }, [])

  const loadMore = useCallback(() => {
    setVisibleCount(prev => Math.min(prev + PAGE_SIZE, toolCalls.length))
  }, [toolCalls.length])

  // Memoize visible items
  const visibleCalls = useMemo(
    () => toolCalls.slice(0, visibleCount),
    [toolCalls, visibleCount]
  )

  const hasMore = visibleCount < toolCalls.length

  if (loading) {
    return (
      <div className={styles.loading}>
        <Spinner size="sm" />
        <span>Loading tool calls...</span>
      </div>
    )
  }

  if (error) {
    return <div className={styles.error}>{error}</div>
  }

  if (toolCalls.length === 0) {
    return <div className={styles.empty}>No tool calls recorded</div>
  }

  return (
    <div className={styles.list}>
      <div className={styles.header}>
        <span className={styles.count}>
          {toolCalls.length} tool calls
          {hasMore && ` (showing ${visibleCount})`}
        </span>
      </div>
      {visibleCalls.map((call, index) => (
        <ToolCallItem
          key={`${call.timestamp}-${index}`}
          call={call}
          index={index}
          expanded={expandedIndex === index}
          onToggle={handleToggle}
        />
      ))}
      {hasMore && (
        <div className={styles.loadMore}>
          <Button variant="ghost" size="sm" onClick={loadMore}>
            Load more ({toolCalls.length - visibleCount} remaining)
          </Button>
        </div>
      )}
    </div>
  )
})

interface ToolCallItemProps {
  call: ToolCallInfo
  index: number
  expanded: boolean
  onToggle: (index: number) => void
}

const ToolCallItem = memo(function ToolCallItem({ call, index, expanded, onToggle }: ToolCallItemProps) {
  const time = formatTime(call.timestamp)
  const toolColor = getToolColor(call.name)

  const handleClick = useCallback(() => {
    onToggle(index)
  }, [onToggle, index])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      onToggle(index)
    }
  }, [onToggle, index])

  return (
    <div className={styles.item}>
      <div
        className={styles.itemHeader}
        onClick={handleClick}
        onKeyDown={handleKeyDown}
        role="button"
        tabIndex={0}
        aria-expanded={expanded}
      >
        <span className={styles.index}>#{index + 1}</span>
        <span className={styles.toolName} style={{ color: toolColor }}>
          {call.name}
        </span>
        <span className={styles.time}>{time}</span>
        <span className={styles.expandIcon} aria-hidden="true">{expanded ? '−' : '+'}</span>
      </div>
      {expanded && (
        <div className={styles.input}>
          <pre>{formatInput(call.input)}</pre>
        </div>
      )}
    </div>
  )
})
