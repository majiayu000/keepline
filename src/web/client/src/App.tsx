import { ThemeProvider } from '@/contexts/ThemeContext'
import { ToastProvider, useToast } from '@/components/Toast'
import { Layout } from '@/components/Layout'
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

  // Keyboard shortcuts
  useKeyboardShortcuts({
    onRefresh: refresh,
    onSync: handleSync,
  })

  async function handleSync() {
    const success = await sync()
    if (success) {
      showToast('Sync completed', 'success')
    } else {
      showToast('Sync failed', 'error')
    }
  }

  async function handleRecover(sessionId: string) {
    const success = await recoverSession(sessionId)
    if (success) {
      showToast('Session recovered', 'success')
    } else {
      showToast('Failed to recover session', 'error')
    }
  }

  async function handleStop(sessionId: string) {
    const success = await stopSession(sessionId)
    if (success) {
      showToast('Session stopped', 'success')
    } else {
      showToast('Failed to stop session', 'error')
    }
  }

  async function handleComplete(sessionId: string) {
    const success = await completeSession(sessionId)
    if (success) {
      showToast('Session marked as completed', 'success')
    } else {
      showToast('Failed to complete session', 'error')
    }
  }

  return (
    <Layout stats={stats} loading={loading} onSync={handleSync} syncing={syncing}>
      {loading && (
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '0.5rem', padding: '2rem' }}>
          <Spinner size="md" />
          <span style={{ color: 'var(--primary)' }}>Loading sessions...</span>
        </div>
      )}

      {error && !loading && (
        <div style={{
          padding: '1rem',
          background: 'var(--bg-surface)',
          border: '1px solid var(--danger)',
          color: 'var(--danger)',
          marginBottom: '1rem',
        }}>
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
