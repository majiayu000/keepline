/**
 * API response types
 */

import type { Session, SessionStats, ToolCallInfo, SubAgent } from './session'

// Base API response
export interface ApiResponse<T = unknown> {
  success: boolean
  data?: T
  error?: string
  message?: string
}

// Pagination info
export interface PaginationInfo {
  total: number
  limit: number
  offset: number
  hasMore: boolean
}

// GET /api/sessions
export interface SessionsData {
  sessions: Session[]
  stats: SessionStats
  pagination?: PaginationInfo
}

// POST /api/sync
export interface SyncResult {
  discovered: number
  updated: number
  lost: number
}

// GET /api/sessions/:id
export interface SessionDetailData {
  session: Session
  recovery: RecoveryInfo
}

// Recovery info
export interface RecoveryInfo {
  canRecover: boolean
  reason?: string
  sessionFile?: string
}

// POST /api/sessions/:id/recover body
export interface RecoverBody {
  method: 'resume' | 'continue' | 'new'
  openTerminal?: boolean
  skipPermissions?: boolean
}

// POST /api/sessions/:id/stop body
export interface StopBody {
  force?: boolean
}

// GET /api/sessions/:id/process
export interface ProcessStatusData {
  pid?: number
  running: boolean
  status: string
}

// GET /api/sessions/:id/tools
export interface ToolCallsData {
  toolCalls: ToolCallInfo[]
  toolCount: number
}

// GET /api/sessions/:id/details (lazy loaded fields)
export interface SessionDetailsData {
  initialPrompt: string
  lastMessage: string
  lastTool: string
  lastToolInput: string
  currentFile: string
  usageStats: {
    totalInputTokens: number
    totalOutputTokens: number
    totalTokens: number
    totalCost: number
    apiCalls: number
  }
}

// GET /api/sessions/:id/subagents
export interface SubAgentsData {
  parentSessionId?: string
  subAgents: SubAgent[]
  count: number
}

// GET /api/sessions/:id/full (combined endpoint - reduces 3 requests to 1)
export interface SessionFullData {
  details: SessionDetailsData
  tools: ToolCallsData
  subAgents: SubAgentsData
}
