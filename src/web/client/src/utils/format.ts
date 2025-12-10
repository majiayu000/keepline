/**
 * Shared formatting utilities
 */

/** Format relative time (e.g., "2m ago", "1h ago") */
export function formatRelativeTime(dateStr: string): string {
  if (!dateStr) return ''
  const date = new Date(dateStr)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffSec = Math.floor(diffMs / 1000)
  const diffMin = Math.floor(diffSec / 60)
  const diffHour = Math.floor(diffMin / 60)
  const diffDay = Math.floor(diffHour / 24)

  if (diffSec < 60) return 'just now'
  if (diffMin < 60) return `${diffMin}m ago`
  if (diffHour < 24) return `${diffHour}h ago`
  if (diffDay < 7) return `${diffDay}d ago`

  return date.toLocaleDateString()
}

/** Format path for display (truncate home directory, show last 3 parts) */
export function formatPath(path: string): string {
  if (!path) return ''
  const homeMatch = path.match(/^(\/Users\/[^/]+|\/home\/[^/]+)\//)
  if (homeMatch) {
    let displayPath = path.slice(homeMatch[1].length + 1)
    if (displayPath.startsWith('Desktop/code/')) {
      displayPath = displayPath.slice('Desktop/code/'.length)
    }
    const parts = displayPath.split('/').filter(Boolean)
    if (parts.length > 3) {
      return '.../' + parts.slice(-3).join('/')
    }
    return displayPath || path
  }
  return path
}

/** Format timestamp to time string (HH:MM:SS) */
export function formatTime(timestamp: string): string {
  const date = new Date(timestamp)
  return date.toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  })
}

/** Format JSON input for display */
export function formatInput(input: Record<string, unknown>): string {
  try {
    return JSON.stringify(input, null, 2)
  } catch {
    return String(input)
  }
}

/** Format token count (e.g., 1234 -> "1.2k") */
export function formatTokens(count: number): string {
  if (count < 1000) return count.toString()
  if (count < 1000000) return (count / 1000).toFixed(1) + 'k'
  return (count / 1000000).toFixed(2) + 'M'
}

/** Format cost (e.g., 0.0123 -> "$0.01") */
export function formatCost(cost: number): string {
  if (cost < 0.01) return `$${cost.toFixed(4)}`
  if (cost < 1) return `$${cost.toFixed(2)}`
  return `$${cost.toFixed(2)}`
}
