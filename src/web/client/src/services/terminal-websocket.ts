/**
 * Terminal WebSocket Manager
 *
 * Manages the /ws/terminal connection with auth-first protocol.
 * Stored on window for HMR survival (same pattern as dashboard WS).
 */

declare global {
  interface Window {
    __termWsManager?: TerminalWebSocketManager
  }
}

export type TerminalWsStatus = 'disconnected' | 'connecting' | 'connected' | 'authenticated' | 'error'

type MessageHandler = (msg: any) => void
type StatusHandler = (status: TerminalWsStatus) => void

class TerminalWebSocketManager {
  private ws: WebSocket | null = null
  private status: TerminalWsStatus = 'disconnected'
  private messageHandlers = new Set<MessageHandler>()
  private statusHandlers = new Set<StatusHandler>()
  private reconnectTimeout: ReturnType<typeof setTimeout> | null = null
  private reconnectAttempts = 0
  private pingInterval: ReturnType<typeof setInterval> | null = null
  private token: string | null = null
  private isDestroyed = false

  private readonly maxReconnectAttempts = 10
  private readonly reconnectIntervalMs = 3000

  private getUrl(): string {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    return `${protocol}//${window.location.host}/ws/terminal`
  }

  private setStatus(s: TerminalWsStatus) {
    if (this.status !== s) {
      this.status = s
      this.statusHandlers.forEach(h => h(s))
    }
  }

  connect(token: string): void {
    if (this.isDestroyed) return
    if (this.ws?.readyState === WebSocket.OPEN || this.ws?.readyState === WebSocket.CONNECTING) return

    this.token = token
    if (this.ws) {
      this.ws.onclose = null
      this.ws.close()
      this.ws = null
    }

    this.setStatus('connecting')

    try {
      const ws = new WebSocket(this.getUrl())

      ws.onopen = () => {
        if (this.isDestroyed) { ws.close(); return }
        this.setStatus('connected')
        this.reconnectAttempts = 0
        // Send auth immediately
        ws.send(JSON.stringify({ type: 'auth', data: { token: this.token } }))
        this.startPing()
      }

      ws.onmessage = (event) => {
        if (this.isDestroyed) return
        try {
          const msg = JSON.parse(event.data)
          if (msg.type === 'auth:ok') {
            this.setStatus('authenticated')
          } else if (msg.type === 'auth:error') {
            this.setStatus('error')
          }
          this.messageHandlers.forEach(h => h(msg))
        } catch { /* ignore parse errors */ }
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
    } catch {
      this.setStatus('error')
      this.scheduleReconnect()
    }
  }

  private scheduleReconnect(): void {
    if (this.isDestroyed || this.reconnectTimeout) return
    if (this.reconnectAttempts >= this.maxReconnectAttempts || !this.token) return

    this.reconnectAttempts++
    this.reconnectTimeout = setTimeout(() => {
      this.reconnectTimeout = null
      if (!this.isDestroyed && this.token) {
        this.connect(this.token)
      }
    }, this.reconnectIntervalMs)
  }

  private startPing(): void {
    this.stopPing()
    this.pingInterval = setInterval(() => {
      this.send({ type: 'ping' })
    }, 30000)
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
      this.ws.onclose = null
      this.ws.close()
      this.ws = null
    }
    this.setStatus('disconnected')
  }

  reconnect(): void {
    this.isDestroyed = false
    this.reconnectAttempts = 0
    if (this.ws) {
      this.ws.onclose = null
      this.ws.close()
      this.ws = null
    }
    if (this.token) this.connect(this.token)
  }

  send(data: any): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(data))
    }
  }

  // Typed convenience methods
  createSession(cols: number, rows: number, cwd?: string, resumeSessionId?: string): void {
    this.send({ type: 'term:create', data: { cols, rows, cwd, resumeSessionId } })
  }

  attachSession(sessionId: string): void {
    this.send({ type: 'term:attach', data: { sessionId } })
  }

  writeInput(sessionId: string, data: string): void {
    this.send({ type: 'term:input', data: { sessionId, data } })
  }

  resizeSession(sessionId: string, cols: number, rows: number): void {
    this.send({ type: 'term:resize', data: { sessionId, cols, rows } })
  }

  detachSession(sessionId: string): void {
    this.send({ type: 'term:detach', data: { sessionId } })
  }

  killSession(sessionId: string): void {
    this.send({ type: 'term:kill', data: { sessionId } })
  }

  listSessions(): void {
    this.send({ type: 'term:list' })
  }

  getStatus(): TerminalWsStatus { return this.status }

  onMessage(handler: MessageHandler): () => void {
    this.messageHandlers.add(handler)
    return () => this.messageHandlers.delete(handler)
  }

  onStatusChange(handler: StatusHandler): () => void {
    this.statusHandlers.add(handler)
    handler(this.status)
    return () => this.statusHandlers.delete(handler)
  }
}

export function getTerminalWsManager(): TerminalWebSocketManager {
  if (typeof window === 'undefined') return new TerminalWebSocketManager()
  if (!window.__termWsManager) {
    window.__termWsManager = new TerminalWebSocketManager()
  }
  return window.__termWsManager
}

if (import.meta.hot) {
  import.meta.hot.accept()
}
