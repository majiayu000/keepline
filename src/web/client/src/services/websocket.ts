/**
 * WebSocket Manager - Singleton pattern to prevent connection leaks
 *
 * This module manages a single WebSocket connection at the module level,
 * avoiding React lifecycle issues that can cause connection leaks.
 *
 * IMPORTANT: The singleton is stored on `window` to survive Vite HMR reloads.
 */

// Extend Window interface for TypeScript
declare global {
  interface Window {
    __wsManager?: WebSocketManager
    __wsAutoConnectCalled?: boolean
    __wsModuleLoadCount?: number
  }
}

// Debug: Track module loads
if (typeof window !== 'undefined') {
  window.__wsModuleLoadCount = (window.__wsModuleLoadCount || 0) + 1
  console.log(`[WS DEBUG] Module loaded (count: ${window.__wsModuleLoadCount})`)
}

export type WebSocketStatus = 'connecting' | 'connected' | 'disconnected' | 'error'

export interface WebSocketMessage {
  type: string
  data?: unknown
  sessionId?: string
  timestamp: string
}

type MessageHandler = (message: WebSocketMessage) => void
type StatusHandler = (status: WebSocketStatus) => void

// Generate unique instance ID
let instanceCounter = 0

class WebSocketManager {
  private ws: WebSocket | null = null
  private status: WebSocketStatus = 'disconnected'
  private reconnectAttempts = 0
  private reconnectTimeout: ReturnType<typeof setTimeout> | null = null
  private pingInterval: ReturnType<typeof setInterval> | null = null
  private messageHandlers = new Set<MessageHandler>()
  private statusHandlers = new Set<StatusHandler>()
  private isDestroyed = false
  private instanceId: number
  private connectCallCount = 0

  private readonly maxReconnectAttempts = 10
  private readonly reconnectInterval = 3000
  private readonly pingIntervalMs = 30000

  constructor() {
    this.instanceId = ++instanceCounter
    console.log(`[WS DEBUG] WebSocketManager constructor called (instance #${this.instanceId})`)
    // Bind methods
    this.connect = this.connect.bind(this)
    this.disconnect = this.disconnect.bind(this)
    this.send = this.send.bind(this)
  }

  private getWebSocketUrl(): string {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const host = window.location.host
    return `${protocol}//${host}/ws`
  }

  private setStatus(newStatus: WebSocketStatus) {
    if (this.status !== newStatus) {
      this.status = newStatus
      this.statusHandlers.forEach(handler => handler(newStatus))
    }
  }

  connect(): void {
    this.connectCallCount++
    const callId = this.connectCallCount
    console.log(`[WS DEBUG] connect() called (instance #${this.instanceId}, call #${callId})`, {
      isDestroyed: this.isDestroyed,
      wsState: this.ws?.readyState,
      currentStatus: this.status,
    })

    // Prevent multiple connections
    if (this.isDestroyed) {
      console.log(`[WS DEBUG] connect() aborted: isDestroyed (call #${callId})`)
      return
    }
    if (this.ws?.readyState === WebSocket.OPEN) {
      console.log(`[WS DEBUG] connect() aborted: already OPEN (call #${callId})`)
      return
    }
    if (this.ws?.readyState === WebSocket.CONNECTING) {
      console.log(`[WS DEBUG] connect() aborted: already CONNECTING (call #${callId})`)
      return
    }

    // Close any existing connection first
    if (this.ws) {
      console.log(`[WS DEBUG] Closing existing connection (call #${callId})`)
      this.ws.onclose = null // Prevent reconnect loop
      this.ws.close()
      this.ws = null
    }

    this.setStatus('connecting')

    try {
      console.log(`[WS DEBUG] Creating new WebSocket (call #${callId})`)
      const ws = new WebSocket(this.getWebSocketUrl())

      ws.onopen = () => {
        if (this.isDestroyed) {
          ws.close()
          return
        }
        this.setStatus('connected')
        this.reconnectAttempts = 0
        this.startPing()
      }

      ws.onmessage = (event) => {
        if (this.isDestroyed) return
        try {
          const message = JSON.parse(event.data) as WebSocketMessage
          this.messageHandlers.forEach(handler => handler(message))
        } catch (error) {
          console.error('Failed to parse WebSocket message:', error)
        }
      }

      ws.onclose = () => {
        if (this.isDestroyed) return
        this.setStatus('disconnected')
        this.ws = null
        this.stopPing()
        this.scheduleReconnect()
      }

      ws.onerror = () => {
        if (this.isDestroyed) return
        this.setStatus('error')
      }

      this.ws = ws
    } catch (error) {
      console.error('Failed to create WebSocket:', error)
      this.setStatus('error')
      this.scheduleReconnect()
    }
  }

  private scheduleReconnect(): void {
    console.log(`[WS DEBUG] scheduleReconnect() called (instance #${this.instanceId})`, {
      isDestroyed: this.isDestroyed,
      hasTimeout: !!this.reconnectTimeout,
      attempts: this.reconnectAttempts,
    })
    if (this.isDestroyed) return
    if (this.reconnectTimeout) return // Already scheduled
    if (this.reconnectAttempts >= this.maxReconnectAttempts) return

    this.reconnectAttempts++
    console.log(`[WS DEBUG] Scheduling reconnect in ${this.reconnectInterval}ms (attempt ${this.reconnectAttempts})`)
    this.reconnectTimeout = setTimeout(() => {
      this.reconnectTimeout = null
      if (!this.isDestroyed) {
        this.connect()
      }
    }, this.reconnectInterval)
  }

  private startPing(): void {
    this.stopPing()
    this.pingInterval = setInterval(() => {
      this.send({ type: 'ping' })
    }, this.pingIntervalMs)
  }

  private stopPing(): void {
    if (this.pingInterval) {
      clearInterval(this.pingInterval)
      this.pingInterval = null
    }
  }

  disconnect(): void {
    this.isDestroyed = true
    this.stopPing()

    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout)
      this.reconnectTimeout = null
    }

    if (this.ws) {
      this.ws.onclose = null // Prevent reconnect
      this.ws.close()
      this.ws = null
    }

    this.setStatus('disconnected')
  }

  send(data: unknown): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(data))
    }
  }

  subscribe(sessionId: string): void {
    this.send({ type: 'subscribe', sessionId })
  }

  unsubscribe(sessionId: string): void {
    this.send({ type: 'unsubscribe', sessionId })
  }

  getStatus(): WebSocketStatus {
    return this.status
  }

  onMessage(handler: MessageHandler): () => void {
    this.messageHandlers.add(handler)
    return () => this.messageHandlers.delete(handler)
  }

  onStatusChange(handler: StatusHandler): () => void {
    this.statusHandlers.add(handler)
    // Immediately call with current status
    handler(this.status)
    return () => this.statusHandlers.delete(handler)
  }

  reconnect(): void {
    this.isDestroyed = false
    this.reconnectAttempts = 0
    if (this.ws) {
      this.ws.onclose = null
      this.ws.close()
      this.ws = null
    }
    this.connect()
  }
}

/**
 * Get or create the singleton WebSocket manager.
 * Uses window storage to survive Vite HMR reloads.
 */
export function getWebSocketManager(): WebSocketManager {
  if (typeof window === 'undefined') {
    // SSR fallback - create a new instance (won't be used)
    return new WebSocketManager()
  }

  console.log(`[WS DEBUG] getWebSocketManager() called, existing: ${!!window.__wsManager}`)
  if (!window.__wsManager) {
    console.log('[WS DEBUG] Creating new WebSocketManager singleton on window')
    window.__wsManager = new WebSocketManager()
  }
  return window.__wsManager
}

// Auto-connect when module loads (only in browser, only once)
if (typeof window !== 'undefined') {
  // Check if auto-connect was already called (survives HMR)
  if (!window.__wsAutoConnectCalled) {
    window.__wsAutoConnectCalled = true

    // Delay initial connection to ensure page is ready
    setTimeout(() => {
      console.log('[WS] Auto-connect triggered')
      getWebSocketManager().connect()
    }, 100)

    // Handle page visibility changes (only add listener once)
    document.addEventListener('visibilitychange', () => {
      const manager = getWebSocketManager()
      if (document.visibilityState === 'visible') {
        // Reconnect if disconnected when page becomes visible
        if (manager.getStatus() === 'disconnected') {
          manager.reconnect()
        }
      }
    })

    // Clean up on page unload (only add listener once)
    window.addEventListener('beforeunload', () => {
      window.__wsManager?.disconnect()
    })
  } else {
    console.log('[WS] Skipping auto-connect (HMR reload detected)')
  }
}

// Vite HMR: Preserve WebSocket connection during hot module replacement
if (import.meta.hot) {
  import.meta.hot.accept(() => {
    console.log('[WS] HMR update accepted, connection preserved')
  })
}
