/**
 * Shared constants and mappings
 */

import type { ToastType } from '@/components/Toast'
import type { SessionStatus } from '@/types'
import {
  SESSION_STATUS_ORDER,
  SESSION_STATUS_PRESENTATION,
} from '../../../../domain/session/status-presentation'

/** Auto-refresh interval in milliseconds */
export const REFRESH_INTERVAL_MS = 30000 // 30 seconds

/** API request timeout in milliseconds */
export const API_TIMEOUT_MS = 30000

/** Session status colors */
export const STATUS_COLORS: Record<SessionStatus, string> = {
  running: 'var(--success)',
  waiting: 'var(--warning)',
  idle: 'var(--info)',
  lost: 'var(--danger)',
  completed: 'var(--text-dim)',
}

/** Get status color with fallback */
export function getStatusColor(status: string): string {
  return STATUS_COLORS[status as SessionStatus] || 'var(--text-dim)'
}

/** Session status icons */
export const STATUS_ICONS: Record<SessionStatus, string> = {
  running: SESSION_STATUS_PRESENTATION.running.icon,
  waiting: SESSION_STATUS_PRESENTATION.waiting.icon,
  idle: SESSION_STATUS_PRESENTATION.idle.icon,
  lost: SESSION_STATUS_PRESENTATION.lost.icon,
  completed: SESSION_STATUS_PRESENTATION.completed.icon,
}

/** Session status labels */
export const STATUS_LABELS: Record<SessionStatus, string> = {
  running: SESSION_STATUS_PRESENTATION.running.shortLabel,
  waiting: SESSION_STATUS_PRESENTATION.waiting.shortLabel,
  idle: SESSION_STATUS_PRESENTATION.idle.shortLabel,
  lost: SESSION_STATUS_PRESENTATION.lost.shortLabel,
  completed: SESSION_STATUS_PRESENTATION.completed.shortLabel,
}

/** Session status order */
export const STATUS_ORDER: readonly SessionStatus[] = SESSION_STATUS_ORDER

/** Tool name colors */
export const TOOL_COLORS: Record<string, string> = {
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

/** Get tool color with fallback */
export function getToolColor(toolName: string): string {
  return TOOL_COLORS[toolName] || 'var(--text)'
}

/** Toast type icons */
export const TOAST_ICONS: Record<ToastType, string> = {
  success: '✓',
  error: '✗',
  warning: '⚠',
  info: 'ℹ',
}

/** Get toast icon */
export function getToastIcon(type: ToastType): string {
  return TOAST_ICONS[type]
}

/** API endpoints */
export const API_BASE = '/api'
