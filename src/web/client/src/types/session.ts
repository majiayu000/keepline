/**
 * Session types - mirrors backend types from core/types.ts
 */

export type SessionStatus = 'running' | 'waiting' | 'idle' | 'lost' | 'completed'

/** Tool call info */
export interface ToolCallInfo {
  name: string
  input: Record<string, unknown>
  timestamp: string
}

export interface Session {
  id: string
  sessionId: string
  directory: string
  status: SessionStatus
  title: string
  initialPrompt: string
  lastTool?: string
  lastToolInput?: string
  currentFile?: string
  lastMessage?: string
  startedAt?: string
  lastActiveAt: string
  completedAt?: string
  pid?: number
  tty?: string
  toolCount: number
  messageCount: number
  createdAt: string
  updatedAt: string
  // Aggregated fields from API
  processRunning?: boolean
  cpuUsage?: number
  memoryUsage?: number
}

export interface SessionStats {
  total: number
  running: number
  waiting: number
  idle: number
  lost: number
  completed: number
}

export const STATUS_ICONS: Record<SessionStatus, string> = {
  running: '▶',
  waiting: '⏸',
  idle: '◇',
  lost: '✕',
  completed: '✓',
}

export const STATUS_LABELS: Record<SessionStatus, string> = {
  running: 'EXEC',
  waiting: 'WAIT',
  idle: 'IDLE',
  lost: 'LOST',
  completed: 'DONE',
}
