import { useCallback, useState, useRef, useEffect, lazy, Suspense } from 'react'
import { ThemeProvider, useTheme, type Theme } from '@/contexts/ThemeContext'
import { ToastProvider, useToast } from '@/components/Toast'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { Layout, layoutStyles } from '@/components/Layout'
import { SessionCardSkeleton } from '@/components/Skeleton'
import { HelpModal } from '@/components/HelpModal'
import { CostPanel } from '@/components/CostPanel'
import { AnalyticsPanel } from '@/components/AnalyticsPanel'
import { useSessions, useKeyboardShortcuts, useSessionFilter, useNotifications } from '@/hooks'

// Lazy load heavy components
const SessionList = lazy(() => import('@/components/SessionList').then(m => ({ default: m.SessionList })))

const THEME_ORDER: Theme[] = ['cyberpunk', 'matrix', 'synthwave', 'minimal', 'tokyo']

function AppContent() {
  const { showToast } = useToast()
  const { theme, setTheme } = useTheme()
  const [showHelp, setShowHelp] = useState(false)

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

  // Search & Filter
  const {
    searchQuery,
    setSearchQuery,
    statusFilters,
    setStatusFilters,
    filteredSessions,
  } = useSessionFilter(sessions)

  // Notifications
  const {
    settings: notificationSettings,
    updateSettings: updateNotificationSettings,
    permission: notificationPermission,
    requestPermission: requestNotificationPermission,
    checkSessionChanges,
  } = useNotifications()

  // Track previous sessions for notification comparison
  const prevSessionsRef = useRef<typeof sessions>([])
  useEffect(() => {
    if (sessions.length > 0 && prevSessionsRef.current.length > 0) {
      checkSessionChanges(prevSessionsRef.current, sessions)
    }
    prevSessionsRef.current = sessions
  }, [sessions, checkSessionChanges])

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

  const handleSetTheme = useCallback((newTheme: Theme) => {
    setTheme(newTheme)
    showToast(`Theme: ${newTheme}`, 'info')
  }, [setTheme, showToast])

  const handleCycleTheme = useCallback(() => {
    const currentIndex = THEME_ORDER.indexOf(theme)
    const nextIndex = (currentIndex + 1) % THEME_ORDER.length
    const nextTheme = THEME_ORDER[nextIndex]
    setTheme(nextTheme)
    showToast(`Theme: ${nextTheme}`, 'info')
  }, [theme, setTheme, showToast])

  // Keyboard shortcuts
  useKeyboardShortcuts({
    onRefresh: refresh,
    onSync: handleSync,
    onShowHelp: () => setShowHelp(true),
    onSetTheme: handleSetTheme,
    onCycleTheme: handleCycleTheme,
  })

  return (
    <Layout
      stats={stats}
      loading={loading}
      onSync={handleSync}
      syncing={syncing}
      searchQuery={searchQuery}
      onSearchChange={setSearchQuery}
      statusFilters={statusFilters}
      onFilterChange={setStatusFilters}
      totalCount={sessions.length}
      filteredCount={filteredSessions.length}
      sessions={filteredSessions}
      notificationSettings={notificationSettings}
      onUpdateNotificationSettings={updateNotificationSettings}
      notificationPermission={notificationPermission}
      onRequestNotificationPermission={requestNotificationPermission}
    >
      {loading && <SessionCardSkeleton count={4} />}

      {error && !loading && (
        <div className={layoutStyles.errorBox} role="alert">
          Error: {error}
        </div>
      )}

      {!loading && filteredSessions.length > 0 && (
        <>
          <CostPanel sessions={filteredSessions} />
          <AnalyticsPanel sessions={filteredSessions} />
        </>
      )}

      {!loading && (
        <Suspense fallback={<SessionCardSkeleton count={4} />}>
          <SessionList
            sessions={filteredSessions}
            onRecover={handleRecover}
            onStop={handleStop}
            onComplete={handleComplete}
            getSessionDetails={getSessionDetails}
            loadSessionDetails={loadSessionDetails}
            isLoadingDetails={isLoadingDetails}
          />
        </Suspense>
      )}

      <HelpModal isOpen={showHelp} onClose={() => setShowHelp(false)} />
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
