/**
 * TerminalPanel - Main terminal tab content
 *
 * Shows existing agent sessions in sidebar.
 * Click to resume through the owning agent CLI.
 * Also supports creating new sessions.
 */

import { useCallback, useMemo, useState, useEffect } from 'react'
import { useTerminal } from '@/hooks/useTerminal'
import { XTermView } from '@/components/XTermView'
import { fetchSessions } from '@/services/api'
import type { Session } from '@/types'
import styles from './TerminalPanel.module.css'

interface TerminalPanelProps {
  token: string
  onLogout: () => Promise<void>
}

export function TerminalPanel({ token, onLogout }: TerminalPanelProps) {
  const terminal = useTerminal(token)
  const [agentSessions, setAgentSessions] = useState<Session[]>([])
  const [loadingSessions, setLoadingSessions] = useState(false)

  // Fetch existing agent sessions
  useEffect(() => {
    setLoadingSessions(true)
    fetchSessions('basic', { limit: 50 }).then(res => {
      if (res.success && res.data) {
        setAgentSessions(res.data.sessions)
      }
      setLoadingSessions(false)
    })
  }, [token])

  // Handlers
  const handleCreate = useCallback(() => {
    terminal.createSession(80, 24)
  }, [terminal])

  const handleResume = useCallback((session: Session) => {
    terminal.createSession(80, 24, session.directory, session.sessionId, session.client)
  }, [terminal])

  const handleKill = useCallback((sessionId: string) => {
    terminal.killSession(sessionId)
  }, [terminal])

  const handleInput = useCallback((data: string) => {
    if (terminal.activeSessionId) {
      terminal.sendInput(terminal.activeSessionId, data)
    }
  }, [terminal])

  const handleResize = useCallback((cols: number, rows: number) => {
    if (terminal.activeSessionId) {
      terminal.resize(terminal.activeSessionId, cols, rows)
    }
  }, [terminal])

  const disconnected = useMemo(
    () => terminal.wsStatus !== 'authenticated',
    [terminal.wsStatus]
  )

  return (
    <div className={styles.container}>
      <div className={styles.sidebar}>
        {/* Agent sessions from API */}
        <div className={styles.sidebarHeader}>
          <span className={styles.sidebarTitle}>History Sessions</span>
          <button className={styles.newBtn} onClick={handleCreate} title="New terminal">+</button>
        </div>
        <div className={styles.sessionList}>
          {loadingSessions && <div className={styles.empty}>Loading...</div>}
          {!loadingSessions && agentSessions.length === 0 && (
            <div className={styles.empty}>No sessions found</div>
          )}
          {agentSessions.map(s => (
            <div
              key={s.sessionId}
              className={styles.sessionItem}
              onClick={() => handleResume(s)}
              title={`Resume: ${s.sessionId}\n${s.directory}`}
            >
              <span
                className={styles.sessionStatus}
                data-status={s.processRunning ? 'running' : 'exited'}
              />
              <span className={styles.sessionId}>
                {s.title || s.sessionId.slice(0, 8)}
              </span>
              <span className={styles.clientTag}>{s.client}</span>
            </div>
          ))}
        </div>

        {/* Active PTY sessions */}
        {terminal.sessions.length > 0 && (
          <>
            <div className={styles.sidebarHeader}>
              <span className={styles.sidebarTitle}>Live Terminals</span>
            </div>
            <div className={styles.sessionList}>
              {terminal.sessions.map(s => (
                <div
                  key={s.id}
                  className={`${styles.sessionItem} ${s.id === terminal.activeSessionId ? styles.active : ''}`}
                  onClick={() => terminal.setActiveSessionId(s.id)}
                >
                  <span className={styles.sessionStatus} data-status={s.status} />
                  <span className={styles.sessionId}>{s.id.slice(0, 8)}</span>
                  <button
                    className={styles.killBtn}
                    onClick={(e) => { e.stopPropagation(); handleKill(s.id) }}
                  >
                    kill
                  </button>
                </div>
              ))}
            </div>
          </>
        )}

        <div className={styles.sidebarFooter}>
          <span className={styles.wsStatus} data-status={terminal.wsStatus}>
            {terminal.wsStatus}
          </span>
          <button className={styles.logoutBtn} onClick={() => onLogout()}>Logout</button>
        </div>
      </div>

      <div className={styles.main}>
        {terminal.activeSessionId ? (
          <XTermView
            sessionId={terminal.activeSessionId}
            onInput={handleInput}
            onResize={handleResize}
            registerOutput={terminal.registerOutputHandler}
            disconnected={disconnected}
          />
        ) : (
          <div className={styles.emptyState}>
            <p>Select an agent session to resume, or create a new terminal.</p>
            <button className={styles.createBtn} onClick={handleCreate}>
              New Terminal
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
