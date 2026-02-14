/**
 * Terminal-related types
 */

export interface AuthStatus {
  setupComplete: boolean
  authenticated: boolean
  username?: string
}

export interface LoginRequest {
  username: string
  password: string
  totpCode?: string
}

export interface SetupRequest {
  username: string
  password: string
  enableTotp?: boolean
}

export interface SetupResponse {
  token: string
  totpUri?: string
}

export interface LoginResponse {
  token: string
}

export interface TerminalSessionInfo {
  id: string
  pid: number
  cwd: string
  status: 'running' | 'exited'
  exitCode?: number
  createdAt: string
  clientCount: number
}
