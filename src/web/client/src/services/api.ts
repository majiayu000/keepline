/**
 * API service - typed fetch wrappers for backend endpoints
 */

import type {
  ApiResponse,
  SessionsData,
  SyncResult,
  SessionDetailData,
  SessionDetailsData,
  RecoverBody,
  StopBody,
  ProcessStatusData,
  ToolCallsData,
  SubAgentsData,
} from '@/types'
import { API_BASE, API_TIMEOUT_MS } from '@/constants'

interface RequestOptions extends RequestInit {
  timeout?: number
}

async function request<T>(
  endpoint: string,
  options?: RequestOptions,
  signal?: AbortSignal
): Promise<ApiResponse<T>> {
  const { timeout = API_TIMEOUT_MS, ...fetchOptions } = options || {}

  // Create timeout controller if no external signal provided
  const timeoutController = new AbortController()
  const timeoutId = setTimeout(() => timeoutController.abort(), timeout)

  // Combine external signal with timeout
  const combinedSignal = signal
    ? combineAbortSignals(signal, timeoutController.signal)
    : timeoutController.signal

  try {
    const response = await fetch(`${API_BASE}${endpoint}`, {
      ...fetchOptions,
      signal: combinedSignal,
      headers: {
        'Content-Type': 'application/json',
        ...fetchOptions?.headers,
      },
    })
    clearTimeout(timeoutId)
    return await response.json()
  } catch (error) {
    clearTimeout(timeoutId)

    if (error instanceof Error) {
      if (error.name === 'AbortError') {
        return {
          success: false,
          error: signal?.aborted ? 'Request cancelled' : 'Request timeout',
        }
      }
      return { success: false, error: error.message }
    }
    return { success: false, error: 'Unknown error' }
  }
}

/** Combine multiple abort signals into one */
function combineAbortSignals(...signals: AbortSignal[]): AbortSignal {
  const controller = new AbortController()
  for (const signal of signals) {
    if (signal.aborted) {
      controller.abort()
      break
    }
    signal.addEventListener('abort', () => controller.abort(), { once: true })
  }
  return controller.signal
}

/**
 * GET /api/sessions - Fetch all sessions with stats
 * @param fields - 'basic' for list view (faster), 'full' for all fields
 */
export async function fetchSessions(
  fields: 'basic' | 'full' = 'basic',
  signal?: AbortSignal
): Promise<ApiResponse<SessionsData>> {
  return request<SessionsData>(`/sessions?fields=${fields}`, undefined, signal)
}

/**
 * POST /api/sync - Sync sessions with filesystem
 */
export async function syncSessions(signal?: AbortSignal): Promise<ApiResponse<SyncResult>> {
  return request<SyncResult>('/sync', { method: 'POST' }, signal)
}

/**
 * GET /api/sessions/:id - Get single session details
 */
export async function fetchSession(
  sessionId: string,
  signal?: AbortSignal
): Promise<ApiResponse<SessionDetailData>> {
  return request<SessionDetailData>(`/sessions/${sessionId}`, undefined, signal)
}

/**
 * POST /api/sessions/:id/recover - Recover a lost session
 */
export async function recoverSession(
  sessionId: string,
  body: RecoverBody,
  signal?: AbortSignal
): Promise<ApiResponse> {
  return request(`/sessions/${sessionId}/recover`, {
    method: 'POST',
    body: JSON.stringify(body),
  }, signal)
}

/**
 * POST /api/sessions/:id/complete - Mark session as completed
 */
export async function completeSession(
  sessionId: string,
  signal?: AbortSignal
): Promise<ApiResponse> {
  return request(`/sessions/${sessionId}/complete`, { method: 'POST' }, signal)
}

/**
 * POST /api/sessions/:id/stop - Stop session process
 */
export async function stopSession(
  sessionId: string,
  body: StopBody = {},
  signal?: AbortSignal
): Promise<ApiResponse> {
  return request(`/sessions/${sessionId}/stop`, {
    method: 'POST',
    body: JSON.stringify(body),
  }, signal)
}

/**
 * GET /api/sessions/:id/process - Get process status
 */
export async function fetchProcessStatus(
  sessionId: string,
  signal?: AbortSignal
): Promise<ApiResponse<ProcessStatusData>> {
  return request<ProcessStatusData>(`/sessions/${sessionId}/process`, undefined, signal)
}

/**
 * GET /api/sessions/:id/tools - Get tool calls for a session
 */
export async function fetchToolCalls(
  sessionId: string,
  signal?: AbortSignal
): Promise<ApiResponse<ToolCallsData>> {
  return request<ToolCallsData>(`/sessions/${sessionId}/tools`, undefined, signal)
}

/**
 * GET /api/sessions/:id/details - Get lazy loaded session details
 */
export async function fetchSessionDetails(
  sessionId: string,
  signal?: AbortSignal
): Promise<ApiResponse<SessionDetailsData>> {
  return request<SessionDetailsData>(`/sessions/${sessionId}/details`, undefined, signal)
}

/**
 * GET /api/sessions/:id/subagents - Get sub-agents for a session
 */
export async function fetchSubAgents(
  sessionId: string,
  signal?: AbortSignal
): Promise<ApiResponse<SubAgentsData>> {
  return request<SubAgentsData>(`/sessions/${sessionId}/subagents`, undefined, signal)
}

// Export all API functions
export const api = {
  fetchSessions,
  syncSessions,
  fetchSession,
  recoverSession,
  completeSession,
  stopSession,
  fetchProcessStatus,
  fetchToolCalls,
  fetchSessionDetails,
  fetchSubAgents,
}
