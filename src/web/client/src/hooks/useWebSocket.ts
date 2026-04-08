import { useState, useEffect, useCallback } from 'react'
import {
  getWebSocketManager,
  type WebSocketStatus,
  type WebSocketMessage,
} from '@/services/websocket'

// Re-export types
export type { WebSocketStatus, WebSocketMessage }

interface UseWebSocketOptions {
  token?: string | null
  onMessage?: (message: WebSocketMessage) => void
  onConnect?: () => void
  onDisconnect?: () => void
}

interface UseWebSocketReturn {
  status: WebSocketStatus
  send: (data: unknown) => void
  subscribe: (sessionId: string) => void
  unsubscribe: (sessionId: string) => void
  reconnect: () => void
}

/**
 * Hook to use the WebSocket singleton manager.
 *
 * This hook connects to the shared WebSocket manager and
 * subscribes to status/message updates. The actual WebSocket
 * connection is managed at the module level to prevent
 * connection leaks from React lifecycle issues.
 */
export function useWebSocket(options: UseWebSocketOptions = {}): UseWebSocketReturn {
  const { token, onMessage, onConnect, onDisconnect } = options
  const [status, setStatus] = useState<WebSocketStatus>('disconnected')

  useEffect(() => {
    const manager = getWebSocketManager()

    if (token) {
      manager.connect(token)
    } else {
      manager.disconnect()
    }

    return () => {
      manager.disconnect()
    }
  }, [token])

  // Subscribe to WebSocket events
  useEffect(() => {
    const manager = getWebSocketManager()

    // Subscribe to status changes
    const unsubscribeStatus = manager.onStatusChange((newStatus) => {
      setStatus(newStatus)

      // Call callbacks
      if (newStatus === 'connected') {
        onConnect?.()
      } else if (newStatus === 'disconnected') {
        onDisconnect?.()
      }
    })

    // Subscribe to messages
    const unsubscribeMessage = onMessage
      ? manager.onMessage(onMessage)
      : () => {}

    return () => {
      unsubscribeStatus()
      unsubscribeMessage()
    }
  }, [onMessage, onConnect, onDisconnect])

  const send = useCallback((data: unknown) => {
    getWebSocketManager().send(data)
  }, [])

  const subscribe = useCallback((sessionId: string) => {
    getWebSocketManager().subscribe(sessionId)
  }, [])

  const unsubscribe = useCallback((sessionId: string) => {
    getWebSocketManager().unsubscribe(sessionId)
  }, [])

  const reconnect = useCallback(() => {
    getWebSocketManager().reconnect()
  }, [])

  return {
    status,
    send,
    subscribe,
    unsubscribe,
    reconnect,
  }
}
