import { useState, useEffect } from 'react'
import type { ToolCallInfo } from '@/types'
import { fetchToolCalls } from '@/services/api'
import { Spinner } from '@/components/Spinner'
import styles from './ToolCallList.module.css'

interface ToolCallListProps {
  sessionId: string
  toolCount?: number
}

export function ToolCallList({ sessionId }: ToolCallListProps) {
  const [toolCalls, setToolCalls] = useState<ToolCallInfo[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [loaded, setLoaded] = useState(false)
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null)

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

  useEffect(() => {
    loadToolCalls()
  }, [sessionId])

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
        <span className={styles.count}>{toolCalls.length} tool calls</span>
      </div>
      {toolCalls.map((call, index) => (
        <ToolCallItem
          key={`${call.timestamp}-${index}`}
          call={call}
          index={index}
          expanded={expandedIndex === index}
          onToggle={() => setExpandedIndex(expandedIndex === index ? null : index)}
        />
      ))}
    </div>
  )
}

interface ToolCallItemProps {
  call: ToolCallInfo
  index: number
  expanded: boolean
  onToggle: () => void
}

function ToolCallItem({ call, index, expanded, onToggle }: ToolCallItemProps) {
  const time = formatTime(call.timestamp)
  const toolColor = getToolColor(call.name)

  return (
    <div className={styles.item}>
      <div className={styles.itemHeader} onClick={onToggle}>
        <span className={styles.index}>#{index + 1}</span>
        <span className={styles.toolName} style={{ color: toolColor }}>
          {call.name}
        </span>
        <span className={styles.time}>{time}</span>
        <span className={styles.expandIcon}>{expanded ? '−' : '+'}</span>
      </div>
      {expanded && (
        <div className={styles.input}>
          <pre>{formatInput(call.input)}</pre>
        </div>
      )}
    </div>
  )
}

function formatTime(timestamp: string): string {
  const date = new Date(timestamp)
  return date.toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  })
}

function formatInput(input: Record<string, unknown>): string {
  try {
    return JSON.stringify(input, null, 2)
  } catch {
    return String(input)
  }
}

function getToolColor(toolName: string): string {
  const colors: Record<string, string> = {
    Read: 'var(--info)',
    Write: 'var(--success)',
    Edit: 'var(--warning)',
    Bash: 'var(--danger)',
    Grep: 'var(--primary)',
    Glob: 'var(--primary)',
    WebFetch: 'var(--info)',
    WebSearch: 'var(--info)',
    Task: 'var(--warning)',
    TodoWrite: 'var(--success)',
  }
  return colors[toolName] || 'var(--text)'
}
