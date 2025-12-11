import type { Session } from '@/types'

/**
 * Export sessions to JSON file
 */
export function exportToJSON(sessions: Session[], filename = 'sessions'): void {
  const data = {
    exportedAt: new Date().toISOString(),
    count: sessions.length,
    sessions: sessions.map(formatSessionForExport),
  }

  const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
  downloadBlob(blob, `${filename}.json`)
}

/**
 * Export sessions to CSV file
 */
export function exportToCSV(sessions: Session[], filename = 'sessions'): void {
  const headers = [
    'Session ID',
    'Title',
    'Status',
    'Directory',
    'Initial Prompt',
    'Last Tool',
    'Tool Count',
    'Message Count',
    'Input Tokens',
    'Output Tokens',
    'Total Tokens',
    'Cost ($)',
    'Started At',
    'Last Active At',
    'Completed At',
  ]

  const rows = sessions.map((session) => [
    session.sessionId,
    escapeCsvField(session.title || ''),
    session.status,
    escapeCsvField(session.directory || ''),
    escapeCsvField(truncate(session.initialPrompt || '', 200)),
    session.lastTool || '',
    session.toolCount.toString(),
    session.messageCount.toString(),
    session.usageStats?.totalInputTokens?.toString() || '0',
    session.usageStats?.totalOutputTokens?.toString() || '0',
    session.usageStats?.totalTokens?.toString() || '0',
    session.usageStats?.totalCost?.toFixed(4) || '0',
    session.startedAt || '',
    session.lastActiveAt || '',
    session.completedAt || '',
  ])

  const csvContent = [
    headers.join(','),
    ...rows.map((row) => row.join(',')),
  ].join('\n')

  const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' })
  downloadBlob(blob, `${filename}.csv`)
}

/**
 * Export sessions to Markdown file
 */
export function exportToMarkdown(sessions: Session[], filename = 'sessions'): void {
  const lines: string[] = [
    '# Claude Code Sessions Report',
    '',
    `> Exported at: ${new Date().toLocaleString()}`,
    `> Total sessions: ${sessions.length}`,
    '',
    '---',
    '',
  ]

  // Group by status
  const grouped = groupByStatus(sessions)

  for (const [status, statusSessions] of Object.entries(grouped)) {
    if (statusSessions.length === 0) continue

    lines.push(`## ${capitalize(status)} (${statusSessions.length})`)
    lines.push('')

    for (const session of statusSessions) {
      lines.push(`### ${session.title || 'Untitled Session'}`)
      lines.push('')
      lines.push(`- **Session ID:** \`${session.sessionId}\``)
      lines.push(`- **Directory:** \`${session.directory || 'N/A'}\``)
      lines.push(`- **Status:** ${session.status}`)
      lines.push(`- **Tools Used:** ${session.toolCount}`)
      lines.push(`- **Messages:** ${session.messageCount}`)

      if (session.usageStats) {
        lines.push(`- **Tokens:** ${session.usageStats.totalTokens.toLocaleString()} (${session.usageStats.totalInputTokens.toLocaleString()} in / ${session.usageStats.totalOutputTokens.toLocaleString()} out)`)
        lines.push(`- **Cost:** $${session.usageStats.totalCost.toFixed(4)}`)
      }

      lines.push(`- **Last Active:** ${formatDate(session.lastActiveAt)}`)

      if (session.initialPrompt) {
        lines.push('')
        lines.push('**Initial Prompt:**')
        lines.push('```')
        lines.push(truncate(session.initialPrompt, 500))
        lines.push('```')
      }

      lines.push('')
      lines.push('---')
      lines.push('')
    }
  }

  const blob = new Blob([lines.join('\n')], { type: 'text/markdown;charset=utf-8;' })
  downloadBlob(blob, `${filename}.md`)
}

// Helper functions

function formatSessionForExport(session: Session) {
  return {
    sessionId: session.sessionId,
    title: session.title,
    status: session.status,
    directory: session.directory,
    initialPrompt: session.initialPrompt,
    lastTool: session.lastTool,
    toolCount: session.toolCount,
    messageCount: session.messageCount,
    usageStats: session.usageStats,
    startedAt: session.startedAt,
    lastActiveAt: session.lastActiveAt,
    completedAt: session.completedAt,
  }
}

function escapeCsvField(field: string): string {
  if (field.includes(',') || field.includes('"') || field.includes('\n')) {
    return `"${field.replace(/"/g, '""')}"`
  }
  return field
}

function truncate(str: string, maxLength: number): string {
  if (str.length <= maxLength) return str
  return str.slice(0, maxLength) + '...'
}

function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
  URL.revokeObjectURL(url)
}

function groupByStatus(sessions: Session[]): Record<string, Session[]> {
  const groups: Record<string, Session[]> = {
    running: [],
    waiting: [],
    idle: [],
    lost: [],
    completed: [],
  }

  for (const session of sessions) {
    if (groups[session.status]) {
      groups[session.status].push(session)
    }
  }

  return groups
}

function capitalize(str: string): string {
  return str.charAt(0).toUpperCase() + str.slice(1)
}

function formatDate(dateStr: string): string {
  if (!dateStr) return 'N/A'
  return new Date(dateStr).toLocaleString()
}
