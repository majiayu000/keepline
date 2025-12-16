import { useCallback, useState, useRef, useEffect, lazy, Suspense } from 'react'
import { ThemeProvider, useTheme, type Theme } from '@/contexts/ThemeContext'
import { ToastProvider, useToast } from '@/components/Toast'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { Layout, layoutStyles } from '@/components/Layout'
import { SessionCardSkeleton } from '@/components/Skeleton'
import { HelpModal } from '@/components/HelpModal'
import { UsagePanel } from '@/components/UsagePanel'
import { ProjectStatsBar } from '@/components/ProjectStatsBar'
import { ProjectsGrid } from '@/components/ProjectsGrid'
import type { TabId } from '@/components/TabNav'
import { useSessions, useKeyboardShortcuts, useSessionFilter, useNotifications, useProjects } from '@/hooks'

// Lazy load heavy components
const SessionList = lazy(() => import('@/components/SessionList').then(m => ({ default: m.SessionList })))

const THEME_ORDER: Theme[] = ['cyberpunk', 'matrix', 'synthwave', 'minimal', 'tokyo']

function AppContent() {
  const { showToast } = useToast()
  const { theme, setTheme } = useTheme()
  const [showHelp, setShowHelp] = useState(false)
  const [activeTab, setActiveTab] = useState<TabId>('sessions')

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
    // Lazy loading - now uses combined /full endpoint (1 request instead of 3)
    getSessionFull,
    loadSessionFull,
    isLoadingFull,
    // Pagination
    pagination,
    loadMore,
    loadingMore,
    // WebSocket
    connectionStatus,
  } = useSessions()

  // Search & Filter
  const {
    searchQuery,
    setSearchQuery,
    statusFilters,
    setStatusFilters,
    filteredSessions,
  } = useSessionFilter(sessions)

  // Projects aggregation - use ALL sessions, not filtered
  // This ensures Projects view always shows all projects regardless of search filter
  const { projects, stats: projectStats } = useProjects(sessions)

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

  const handleRecover = useCallback(async (sessionId: string, terminalApp?: import('@/types').TerminalApp) => {
    const success = await recoverSession(sessionId, terminalApp)
    showToast(
      success ? `Session opened in ${terminalApp || 'terminal'}` : 'Failed to recover session',
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

  const handleProjectClick = useCallback((projectPath: string) => {
    // Switch to sessions tab and filter by project directory
    setActiveTab('sessions')
    setSearchQuery(projectPath)
    showToast(`Filtered to: ${projectPath.split('/').pop()}`, 'info')
  }, [setSearchQuery, showToast])

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
      connectionStatus={connectionStatus}
      activeTab={activeTab}
      onTabChange={setActiveTab}
    >
      {loading && <SessionCardSkeleton count={4} />}

      {error && !loading && (
        <div className={layoutStyles.errorBox} role="alert">
          Error: {error}
        </div>
      )}

      {/* Sessions Tab */}
      {activeTab === 'sessions' && !loading && (
        <Suspense fallback={<SessionCardSkeleton count={4} />}>
          <SessionList
            sessions={filteredSessions}
            onRecover={handleRecover}
            onStop={handleStop}
            onComplete={handleComplete}
            getSessionFull={getSessionFull}
            loadSessionFull={loadSessionFull}
            isLoadingFull={isLoadingFull}
            pagination={pagination}
            onLoadMore={loadMore}
            loadingMore={loadingMore}
          />
        </Suspense>
      )}

      {/* Analytics Tab - use ccusage for accurate data */}
      {activeTab === 'analytics' && !loading && (
        <UsagePanel />
      )}

      {/* Projects Tab */}
      {activeTab === 'projects' && !loading && (
        <>
          <ProjectStatsBar stats={projectStats} />
          <ProjectsGrid
            projects={projects}
            onProjectClick={handleProjectClick}
          />
        </>
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
