import type { AgentClient, SessionStatus } from './session'

export type OrchestratorReasonCode =
  | 'waiting_for_human'
  | 'recoverable_lost'
  | 'high_cost'
  | 'stale_activity'
  | 'idle_activity'
  | 'active_session'

export type OrchestratorSeverity = 'critical' | 'warning' | 'info'
export type OrchestratorRecommendedAction = 'review' | 'recover' | 'monitor' | 'resume' | 'none'
export type OrchestratorDigestSource = 'deterministic' | 'local_model'
export type OrchestratorDigestStatus = 'fresh' | 'stale' | 'error'

export interface OrchestratorReason {
  code: OrchestratorReasonCode
  severity: OrchestratorSeverity
  message: string
  score: number
}

export interface OrchestratorSessionContext {
  initialPrompt?: string
  lastMessage?: string
  lastTool?: string
  currentFile?: string
  messageCount: number
  toolCount: number
}

export interface OrchestratorDigest {
  sessionId: string
  summary: string
  nextActions: string[]
  blockers: string[]
  waitingForHuman: boolean
  source: OrchestratorDigestSource
  status: OrchestratorDigestStatus
  sourceUpdatedAt: string
  generatedAt: string
  provider?: string
  errorMessage?: string
}

export interface OrchestratorQueueItem {
  rank: number
  sessionId: string
  client: AgentClient
  status: SessionStatus
  title: string
  directory: string
  lastActiveAt: string
  score: number
  reasons: OrchestratorReason[]
  recommendedAction: OrchestratorRecommendedAction
  processRunning: boolean
  context: OrchestratorSessionContext
  usageCost?: number
  digest?: OrchestratorDigest
}

export interface OrchestratorOverviewStats {
  totalCandidates: number
  needingAttention: number
  critical: number
  warning: number
  hiddenOldLost: number
  lostWindowHours?: number
}

export interface OrchestratorOverviewData {
  generatedAt: string
  items: OrchestratorQueueItem[]
  stats: OrchestratorOverviewStats
}
