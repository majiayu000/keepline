import { useCallback, lazy, Suspense } from 'react'
import { ThemeProvider } from '@/contexts/ThemeContext'
import { ToastProvider, useToast } from '@/components/Toast'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { Layout, layoutStyles } from '@/components/Layout'
import { SessionCardSkeleton } from '@/components/Skeleton'
import { useSessions, useKeyboardShortcuts } from '@/hooks'

// Lazy load heavy components
const SessionList = lazy(() => import('@/components/SessionList').then(m => ({ default: m.SessionList })))

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
    // Lazy loading
    getSessionDetails,
    loadSessionDetails,
    isLoadingDetails,
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
      {loading && <SessionCardSkeleton count={4} />}

      {error && !loading && (
        <div className={layoutStyles.errorBox} role="alert">
          Error: {error}
        </div>
      )}

      {!loading && (
        <Suspense fallback={<SessionCardSkeleton count={4} />}>
          <SessionList
            sessions={sessions}
            onRecover={handleRecover}
            onStop={handleStop}
            onComplete={handleComplete}
            getSessionDetails={getSessionDetails}
            loadSessionDetails={loadSessionDetails}
            isLoadingDetails={isLoadingDetails}
          />
        </Suspense>
      )}
    </Layout>
  )
}

function App() {
  return (
    <ErrorBoundary>
      <ThemeProvider>
        <ToastProvider>
          <AppContent />
        </ToastProvider>
      </ThemeProvider>
    </ErrorBoundary>
  )
}

export default App
