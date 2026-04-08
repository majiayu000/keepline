/**
 * useTerminal - Terminal session management hook
 */

import { useState, useCallback, useEffect, useRef } from 'react'
import { getTerminalWsManager, type TerminalWsStatus } from '@/services/terminal-websocket'
import type { TerminalSessionInfo } from '@/types'

export function useTerminal(token: string | null) {
  const [sessions, setSessions] = useState<TerminalSessionInfo[]>([])
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null)
  const [wsStatus, setWsStatus] = useState<TerminalWsStatus>('disconnected')
  const outputHandlersRef = useRef<Map<string, (data: string) => void>>(new Map())

  // Connect when token is available
  useEffect(() => {
    const manager = getTerminalWsManager()
    if (!token) {
      manager.disconnect()
      return
    }

    manager.connect(token)

    const unsubStatus = manager.onStatusChange(setWsStatus)
    const unsubMsg = manager.onMessage((msg) => {
      switch (msg.type) {
        case 'term:created': {
          const { sessionId, pid } = msg.data
          setSessions(prev => [...prev, {
            id: sessionId,
            pid,
            cwd: '',
            status: 'running',
            createdAt: new Date().toISOString(),
            clientCount: 1,
          }])
          setActiveSessionId(sessionId)
          break
        }
        case 'term:output': {
          const handler = outputHandlersRef.current.get(msg.data.sessionId)
          if (handler) handler(msg.data.data)
          break
        }
        case 'term:scrollback': {
          const handler = outputHandlersRef.current.get(msg.data.sessionId)
          if (handler) handler(msg.data.data)
          break
        }
        case 'term:exited': {
          setSessions(prev => prev.map(s =>
            s.id === msg.data.sessionId
              ? { ...s, status: 'exited', exitCode: msg.data.exitCode }
              : s
          ))
          break
        }
        case 'term:list': {
          setSessions(msg.data.sessions)
          break
        }
        case 'auth:ok': {
          // Request session list on auth
          manager.listSessions()
          break
        }
      }
    })

    return () => {
      unsubStatus()
      unsubMsg()
      manager.disconnect()
    }
  }, [token])

  const createSession = useCallback((cols: number, rows: number, cwd?: string, resumeSessionId?: string) => {
    getTerminalWsManager().createSession(cols, rows, cwd, resumeSessionId)
  }, [])

  const attachSession = useCallback((sessionId: string) => {
    getTerminalWsManager().attachSession(sessionId)
    setActiveSessionId(sessionId)
  }, [])

  const detachSession = useCallback((sessionId: string) => {
    getTerminalWsManager().detachSession(sessionId)
  }, [])

  const killSession = useCallback((sessionId: string) => {
    getTerminalWsManager().killSession(sessionId)
  }, [])

  const sendInput = useCallback((sessionId: string, data: string) => {
    getTerminalWsManager().writeInput(sessionId, data)
  }, [])

  const resize = useCallback((sessionId: string, cols: number, rows: number) => {
    getTerminalWsManager().resizeSession(sessionId, cols, rows)
  }, [])

  const registerOutputHandler = useCallback((sessionId: string, handler: (data: string) => void) => {
    outputHandlersRef.current.set(sessionId, handler)
    return () => { outputHandlersRef.current.delete(sessionId) }
  }, [])

  const refreshSessions = useCallback(() => {
    getTerminalWsManager().listSessions()
  }, [])

  return {
    sessions,
    activeSessionId,
    setActiveSessionId,
    wsStatus,
    createSession,
    attachSession,
    detachSession,
    killSession,
    sendInput,
    resize,
    registerOutputHandler,
    refreshSessions,
  }
}
