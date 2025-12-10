/**
 * Shared constants and mappings
 */

import type { ToastType } from '@/components/Toast'

/** Session status colors */
export const STATUS_COLORS: Record<string, string> = {
  running: 'var(--success)',
  waiting: 'var(--warning)',
  idle: 'var(--info)',
  lost: 'var(--danger)',
  completed: 'var(--text-dim)',
}

/** Get status color with fallback */
export function getStatusColor(status: string): string {
  return STATUS_COLORS[status] || 'var(--text-dim)'
}

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

/** Refresh intervals */
export const REFRESH_INTERVAL_MS = 30000

/** API endpoints */
export const API_BASE = '/api'
