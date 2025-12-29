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

// Terminal app options
export type TerminalApp = 'Terminal' | 'iTerm' | 'Warp' | 'auto'

// POST /api/sessions/:id/recover body
export interface RecoverBody {
  method: 'resume' | 'continue' | 'new'
  openTerminal?: boolean
  skipPermissions?: boolean
  terminalApp?: TerminalApp
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

// GET /api/usage - ccusage data types
export interface ModelBreakdown {
  modelName: string
  inputTokens: number
  outputTokens: number
  cacheCreationTokens: number
  cacheReadTokens: number
  cost: number
}

export interface DailyUsage {
  date: string
  inputTokens: number
  outputTokens: number
  cacheCreationTokens: number
  cacheReadTokens: number
  totalTokens: number
  totalCost: number
  modelsUsed: string[]
  modelBreakdowns: ModelBreakdown[]
}

export interface MonthlyUsage {
  month: string
  inputTokens: number
  outputTokens: number
  cacheCreationTokens: number
  cacheReadTokens: number
  totalTokens: number
  totalCost: number
  modelsUsed: string[]
  modelBreakdowns: ModelBreakdown[]
}

export interface UsageData {
  daily?: DailyUsage[]
  monthly?: MonthlyUsage[]
  weekly?: DailyUsage[] // Same structure as daily
}

export interface ClientQuotaWindow {
  id?: string
  label: string
  utilization: number
  resets_at: string | null
}

export interface ClientDefinition {
  id: string
  name: string
  kind?: string
  status?: string
  note?: string
  quota_windows?: ClientQuotaWindow[]
}

export interface ClientsData {
  clients: ClientDefinition[]
  source_path: string | null
}

// GET /api/codex/quota - Codex CLI rate limit quota
export interface CodexQuotaData {
  session: QuotaWindow
  weekly: QuotaWindow
  email?: string
  plan_type?: string
  limit_reached?: boolean
}

// GET /api/quota - Claude Code rate limit quota
export interface QuotaWindow {
  utilization: number // Percentage used (0-100)
  resets_at: string | null // ISO timestamp when window resets
}

export interface ExtraUsage {
  is_enabled: boolean
  monthly_limit: number | null
  used_credits: number | null
  utilization: number | null
}

export interface QuotaData {
  five_hour: QuotaWindow // 5-hour rolling window
  seven_day: QuotaWindow // 7-day weekly limit
  seven_day_oauth_apps?: QuotaWindow | null
  seven_day_opus?: QuotaWindow | null // Opus-specific limits
  seven_day_sonnet?: QuotaWindow | null // Sonnet-specific limits
  iguana_necktie?: unknown | null // Internal field
  extra_usage?: ExtraUsage | null // Extra usage credits
}
