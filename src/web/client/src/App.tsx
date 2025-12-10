import { useCallback } from 'react'
import { ThemeProvider } from '@/contexts/ThemeContext'
import { ToastProvider, useToast } from '@/components/Toast'
import { Layout, layoutStyles } from '@/components/Layout'
import { SessionList } from '@/components/SessionList'
import { Spinner } from '@/components/Spinner'
import { useSessions, useKeyboardShortcuts } from '@/hooks'

function AppContent() {
  const { showToast } = useToast()
  const {
    sessions,
    stats,
    loading,
    syncing,
    error,
    refresh,
    sync,
    recoverSession,
    stopSession,
    completeSession,
  } = useSessions()

  const handleSync = useCallback(async () => {
    const success = await sync()
    showToast(success ? 'Sync completed' : 'Sync failed', success ? 'success' : 'error')
  }, [sync, showToast])

  const handleRecover = useCallback(async (sessionId: string) => {
    const success = await recoverSession(sessionId)
    showToast(
      success ? 'Session recovered' : 'Failed to recover session',
      success ? 'success' : 'error'
    )
  }, [recoverSession, showToast])

  const handleStop = useCallback(async (sessionId: string) => {
    const success = await stopSession(sessionId)
    showToast(
      success ? 'Session stopped' : 'Failed to stop session',
      success ? 'success' : 'error'
    )
  }, [stopSession, showToast])

  const handleComplete = useCallback(async (sessionId: string) => {
    const success = await completeSession(sessionId)
    showToast(
      success ? 'Session marked as completed' : 'Failed to complete session',
      success ? 'success' : 'error'
    )
  }, [completeSession, showToast])

  // Keyboard shortcuts
  useKeyboardShortcuts({
    onRefresh: refresh,
    onSync: handleSync,
  })

  return (
    <Layout stats={stats} loading={loading} onSync={handleSync} syncing={syncing}>
      {loading && (
        <div className={layoutStyles.loadingContainer}>
          <Spinner size="md" />
          <span className={layoutStyles.loadingText}>Loading sessions...</span>
        </div>
      )}

      {error && !loading && (
        <div className={layoutStyles.errorBox}>
          Error: {error}
        </div>
      )}

      {!loading && (
        <SessionList
          sessions={sessions}
          onRecover={handleRecover}
          onStop={handleStop}
          onComplete={handleComplete}
        />
      )}
    </Layout>
  )
}

function App() {
  return (
    <ThemeProvider>
      <ToastProvider>
        <AppContent />
      </ToastProvider>
    </ThemeProvider>
  )
}

export default App
