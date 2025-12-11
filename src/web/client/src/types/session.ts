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

/** Usage statistics */
export interface UsageStats {
  totalInputTokens: number
  totalOutputTokens: number
  totalTokens: number
  totalCost: number
  apiCalls: number
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
  // Usage stats
  usageStats?: UsageStats
  // Multi-session tracking
  subAgentCount?: number  // Number of sub-agents spawned by this session
}

/** Sub-agent session info */
export interface SubAgent {
  sessionId: string
  agentId?: string
  directory: string
  firstMessage?: string
  lastMessage?: string
  messageCount: number
  toolCount: number
  lastTool?: string
  startedAt?: string
  lastActiveAt: string
  usageStats?: UsageStats
}

export interface SessionStats {
  total: number
  running: number
  waiting: number
  idle: number
  lost: number
  completed: number
}

// Note: STATUS_ICONS and STATUS_LABELS are now in @/constants
