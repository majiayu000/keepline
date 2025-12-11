import { useState, useEffect, useCallback, useRef } from 'react'

export type WebSocketStatus = 'connecting' | 'connected' | 'disconnected' | 'error'

export interface WebSocketMessage {
  type: string
  data?: unknown
  sessionId?: string
  timestamp: string
}

interface UseWebSocketOptions {
  onMessage?: (message: WebSocketMessage) => void
  onConnect?: () => void
  onDisconnect?: () => void
  reconnectInterval?: number
  maxReconnectAttempts?: number
}

interface UseWebSocketReturn {
  status: WebSocketStatus
  send: (data: unknown) => void
  subscribe: (sessionId: string) => void
  unsubscribe: (sessionId: string) => void
  reconnect: () => void
}

export function useWebSocket(options: UseWebSocketOptions = {}): UseWebSocketReturn {
  const {
    onMessage,
    onConnect,
    onDisconnect,
    reconnectInterval = 3000,
    maxReconnectAttempts = 10,
  } = options

  const [status, setStatus] = useState<WebSocketStatus>('disconnected')
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectAttemptsRef = useRef(0)
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const mountedRef = useRef(true)

  // Store callbacks in refs to avoid re-triggering connect on callback changes
  const onMessageRef = useRef(onMessage)
  const onConnectRef = useRef(onConnect)
  const onDisconnectRef = useRef(onDisconnect)

  // Keep refs updated
  useEffect(() => {
    onMessageRef.current = onMessage
    onConnectRef.current = onConnect
    onDisconnectRef.current = onDisconnect
  })

  const getWebSocketUrl = useCallback(() => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const host = window.location.host
    return `${protocol}//${host}/ws`
  }, [])

  const connect = useCallback(() => {
    // Prevent multiple connections
    if (wsRef.current?.readyState === WebSocket.OPEN ||
        wsRef.current?.readyState === WebSocket.CONNECTING) {
      return
    }

    setStatus('connecting')

    try {
      const ws = new WebSocket(getWebSocketUrl())

      ws.onopen = () => {
        if (!mountedRef.current) {
          ws.close()
          return
        }
        setStatus('connected')
        reconnectAttemptsRef.current = 0
        onConnectRef.current?.()
      }

      ws.onmessage = (event) => {
        if (!mountedRef.current) return
        try {
          const message = JSON.parse(event.data) as WebSocketMessage
          onMessageRef.current?.(message)
        } catch (error) {
          console.error('Failed to parse WebSocket message:', error)
        }
      }

      ws.onclose = () => {
        if (!mountedRef.current) return
        setStatus('disconnected')
        wsRef.current = null
        onDisconnectRef.current?.()

        // Attempt to reconnect
        if (reconnectAttemptsRef.current < maxReconnectAttempts) {
          reconnectAttemptsRef.current++
          reconnectTimeoutRef.current = setTimeout(() => {
            if (mountedRef.current) {
              connect()
            }
          }, reconnectInterval)
        }
      }

      ws.onerror = () => {
        if (!mountedRef.current) return
        setStatus('error')
      }

      wsRef.current = ws
    } catch (error) {
      console.error('Failed to create WebSocket:', error)
      setStatus('error')
    }
  }, [getWebSocketUrl, reconnectInterval, maxReconnectAttempts]) // Removed callback deps

  const send = useCallback((data: unknown) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(data))
    }
  }, [])

  const subscribe = useCallback((sessionId: string) => {
    send({ type: 'subscribe', sessionId })
  }, [send])

  const unsubscribe = useCallback((sessionId: string) => {
    send({ type: 'unsubscribe', sessionId })
  }, [send])

  const reconnect = useCallback(() => {
    if (wsRef.current) {
      wsRef.current.close()
    }
    reconnectAttemptsRef.current = 0
    connect()
  }, [connect])

  // Store connect in ref for cleanup
  const connectRef = useRef(connect)
  useEffect(() => {
    connectRef.current = connect
  }, [connect])

  // Connect on mount - run only once
  useEffect(() => {
    mountedRef.current = true
    connectRef.current()

    // Ping to keep connection alive
    const pingInterval = setInterval(() => {
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        wsRef.current.send(JSON.stringify({ type: 'ping' }))
      }
    }, 30000)

    return () => {
      mountedRef.current = false
      clearInterval(pingInterval)
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current)
      }
      if (wsRef.current) {
        wsRef.current.close()
        wsRef.current = null
      }
    }
  }, []) // Empty deps - only run on mount

  return {
    status,
    send,
    subscribe,
    unsubscribe,
    reconnect,
  }
}
