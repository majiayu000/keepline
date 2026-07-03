import type { Session, SessionStats } from '../types/session'

export interface SessionsUpdateData {
  sessions: Session[]
  stats: SessionStats
}

export interface SyncCompleteData {
  timestamp: string
}

export function isSessionStats(value: unknown): value is SessionStats {
  if (!value || typeof value !== 'object') return false
  const stats = value as Partial<Record<keyof SessionStats, unknown>>
  return typeof stats.total === 'number' &&
    typeof stats.running === 'number' &&
    typeof stats.waiting === 'number' &&
    typeof stats.idle === 'number' &&
    typeof stats.lost === 'number' &&
    typeof stats.completed === 'number'
}

export function isSessionsUpdateData(value: unknown): value is SessionsUpdateData {
  if (!value || typeof value !== 'object') return false
  const data = value as { sessions?: unknown; stats?: unknown }
  return Array.isArray(data.sessions) && isSessionStats(data.stats)
}

export function isSyncCompleteData(value: unknown): value is SyncCompleteData {
  if (!value || typeof value !== 'object') return false
  const data = value as { timestamp?: unknown }
  return typeof data.timestamp === 'string' && data.timestamp.length > 0
}
