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
import { MemoryPanel } from '@/components/MemoryPanel'
import { PlansPanel } from '@/components/PlansPanel'
import { TerminalPanel } from '@/components/TerminalPanel'
import { AuthSetup } from '@/components/AuthSetup'
import { AuthLogin } from '@/components/AuthLogin'
import type { TabId } from '@/components/TabNav'
import { useAuth, useSessions, useKeyboardShortcuts, useSessionFilter, useNotifications, useProjects } from '@/hooks'
import type { ProjectInfo } from '@/types'

// Lazy load heavy components
const SessionList = lazy(() => import('@/components/SessionList').then(m => ({ default: m.SessionList })))

const THEME_ORDER: Theme[] = ['cyberpunk', 'matrix', 'synthwave', 'minimal', 'tokyo']

interface DashboardAppProps {
  token: string
  onLogout: () => Promise<void>
}

function DashboardApp({ token, onLogout }: DashboardAppProps) {
  const { showToast } = useToast()
  const { theme, setTheme } = useTheme()
  const [showHelp, setShowHelp] = useState(false)
  const [activeTab, setActiveTab] = useState<TabId>('sessions')
  const [selectedProjectRoot, setSelectedProjectRoot] = useState<string | null>(null)

  const {
    sessions,
    allSessions,
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
    version: sessionsVersion,
  } = useSessions(token, { projectRoot: selectedProjectRoot ?? undefined })

  // Search & Filter
  const {
    searchQuery,
    setSearchQuery,
    statusFilters,
    setStatusFilters,
    filteredSessions,
  } = useSessionFilter(sessions)

  // Projects come from the backend project API, not the paginated sessions list.
  const {
    projects,
    stats: projectStats,
    loading: projectsLoading,
    error: projectsError,
  } = useProjects(token, sessionsVersion)

  const selectedProject = selectedProjectRoot
    ? projects.find(project => project.rootPath === selectedProjectRoot)
    : undefined

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
    const notificationSessions = allSessions.length > 0 ? allSessions : sessions
    if (notificationSessions.length > 0 && prevSessionsRef.current.length > 0) {
      checkSessionChanges(prevSessionsRef.current, notificationSessions)
    }
    prevSessionsRef.current = notificationSessions
  }, [allSessions, sessions, checkSessionChanges])

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

  const handleProjectClick = useCallback((project: ProjectInfo) => {
    setSelectedProjectRoot(project.rootPath)
    setSearchQuery('')
    setActiveTab('sessions')
    showToast(`Filtered to: ${project.name}`, 'info')
  }, [setSearchQuery, showToast])

  const handleClearProjectFilter = useCallback(() => {
    setSelectedProjectRoot(null)
    showToast('Project filter cleared', 'info')
  }, [showToast])

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
          {selectedProjectRoot && (
            <div className={layoutStyles.projectFilterBar}>
              <span className={layoutStyles.projectFilterText}>
                Project:
                <strong>{selectedProject?.name || selectedProjectRoot.split('/').pop() || selectedProjectRoot}</strong>
                <span className={layoutStyles.projectFilterPath}>
                  {selectedProject?.displayPath || selectedProjectRoot}
                </span>
              </span>
              <button
                type="button"
                className={layoutStyles.projectFilterClear}
                onClick={handleClearProjectFilter}
              >
                Clear
              </button>
            </div>
          )}
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
          {projectsError && (
            <div className={layoutStyles.errorBox} role="alert">
              Error: {projectsError}
            </div>
          )}
          <ProjectStatsBar stats={projectStats} />
          {projectsLoading ? (
            <SessionCardSkeleton count={4} />
          ) : (
            <ProjectsGrid
              projects={projects}
              onProjectClick={handleProjectClick}
            />
          )}
        </>
      )}

      {/* Memory Tab */}
      {activeTab === 'memory' && !loading && (
        <MemoryPanel />
      )}

      {/* Plans Tab */}
      {activeTab === 'plans' && !loading && (
        <PlansPanel />
      )}

      {/* Terminal Tab */}
      {activeTab === 'terminal' && (
        <TerminalPanel token={token} onLogout={onLogout} />
      )}

      <HelpModal isOpen={showHelp} onClose={() => setShowHelp(false)} />
    </Layout>
  )
}

function AppContent() {
  const auth = useAuth()
  const token = auth.getToken()

  if (auth.loading || !auth.status) {
    return <SessionCardSkeleton count={4} />
  }

  if (!auth.status.setupComplete) {
    return <AuthSetup onSetup={auth.setup} error={auth.error} />
  }

  if (!auth.status.authenticated || !token) {
    return <AuthLogin onLogin={auth.login} onLocalLogin={auth.localLogin} error={auth.error} />
  }

  return <DashboardApp token={token} onLogout={auth.logout} />
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
