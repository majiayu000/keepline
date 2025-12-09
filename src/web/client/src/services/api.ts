/**
 * API service - typed fetch wrappers for backend endpoints
 */

import type {
  ApiResponse,
  SessionsData,
  SyncResult,
  SessionDetailData,
  RecoverBody,
  StopBody,
  ProcessStatusData,
  ToolCallsData,
} from '@/types'

const API_BASE = '/api'

async function request<T>(
  endpoint: string,
  options?: RequestInit
): Promise<ApiResponse<T>> {
  try {
    const response = await fetch(`${API_BASE}${endpoint}`, {
      ...options,
      headers: {
        'Content-Type': 'application/json',
        ...options?.headers,
      },
    })
    return await response.json()
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Unknown error',
    }
  }
}

/**
 * GET /api/sessions - Fetch all sessions with stats
 */
export async function fetchSessions(): Promise<ApiResponse<SessionsData>> {
  return request<SessionsData>('/sessions')
}

/**
 * POST /api/sync - Sync sessions with filesystem
 */
export async function syncSessions(): Promise<ApiResponse<SyncResult>> {
  return request<SyncResult>('/sync', { method: 'POST' })
}

/**
 * GET /api/sessions/:id - Get single session details
 */
export async function fetchSession(
  sessionId: string
): Promise<ApiResponse<SessionDetailData>> {
  return request<SessionDetailData>(`/sessions/${sessionId}`)
}

/**
 * POST /api/sessions/:id/recover - Recover a lost session
 */
export async function recoverSession(
  sessionId: string,
  body: RecoverBody
): Promise<ApiResponse> {
  return request(`/sessions/${sessionId}/recover`, {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

/**
 * POST /api/sessions/:id/complete - Mark session as completed
 */
export async function completeSession(
  sessionId: string
): Promise<ApiResponse> {
  return request(`/sessions/${sessionId}/complete`, { method: 'POST' })
}

/**
 * POST /api/sessions/:id/stop - Stop session process
 */
export async function stopSession(
  sessionId: string,
  body: StopBody = {}
): Promise<ApiResponse> {
  return request(`/sessions/${sessionId}/stop`, {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

/**
 * GET /api/sessions/:id/process - Get process status
 */
export async function fetchProcessStatus(
  sessionId: string
): Promise<ApiResponse<ProcessStatusData>> {
  return request<ProcessStatusData>(`/sessions/${sessionId}/process`)
}

/**
 * GET /api/sessions/:id/tools - Get tool calls for a session
 */
export async function fetchToolCalls(
  sessionId: string
): Promise<ApiResponse<ToolCallsData>> {
  return request<ToolCallsData>(`/sessions/${sessionId}/tools`)
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
}
