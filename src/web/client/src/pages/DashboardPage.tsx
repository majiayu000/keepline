import { lazy, Suspense } from "react";
import { Layout, layoutStyles } from "@/components/Layout";
import { SessionCardSkeleton } from "@/components/Skeleton";
import { HelpModal } from "@/components/HelpModal";

const SessionList = lazy(() =>
  import("@/components/SessionList").then((m) => ({ default: m.SessionList })),
);
const UsagePanel = lazy(() =>
  import("@/components/UsagePanel").then((m) => ({ default: m.UsagePanel })),
);
const ProjectStatsBar = lazy(() =>
  import("@/components/ProjectStatsBar").then((m) => ({
    default: m.ProjectStatsBar,
  })),
);
const ProjectsGrid = lazy(() =>
  import("@/components/ProjectsGrid").then((m) => ({
    default: m.ProjectsGrid,
  })),
);
const MemoryPanel = lazy(() =>
  import("@/components/MemoryPanel").then((m) => ({ default: m.MemoryPanel })),
);
const PlansPanel = lazy(() =>
  import("@/components/PlansPanel").then((m) => ({ default: m.PlansPanel })),
);
const WorkItemsPanel = lazy(() =>
  import("@/components/WorkItemsPanel").then((m) => ({
    default: m.WorkItemsPanel,
  })),
);
const OrchestratorPanel = lazy(() =>
  import("@/components/OrchestratorPanel").then((m) => ({
    default: m.OrchestratorPanel,
  })),
);

import { useDashboard, type DashboardProps } from "@/hooks/useDashboard";

export function DashboardPage(props: DashboardProps) {
  const { activeTab, onNavigate: setActiveTab, onLogout, token } = props;
  const {
    showHelp,
    setShowHelp,
    selectedProjectRoot,
    searchQuery,
    setSearchQuery,
    statusFilters,
    setStatusFilters,
    runtimeFilter,
    setRuntimeFilter,
    expandedSessionId,
    stats,
    loading,
    syncing,
    error,
    filteredSessions,
    totalSessionCount,
    matchedSessionCount,
    hasActiveFilters,
    projects,
    projectStats,
    projectsLoading,
    projectsError,
    selectedProject,
    notificationSettings,
    updateNotificationSettings,
    notificationPermission,
    requestNotificationPermission,
    connectionStatus,
    getSessionFull,
    loadSessionFull,
    isLoadingFull,
    pagination,
    loadMore,
    loadingMore,
    handleSync,
    handleRecover,
    handleStop,
    handleComplete,
    handleProjectClick,
    handleClearProjectFilter,
    handleOpenOrchestratorSession,
    handleInitialExpansionConsumed,
    handleCopyRecoveryCommand,
  } = useDashboard(props);
  return (
    <Layout
      stats={stats}
      loading={loading}
      onSync={handleSync}
      onLogout={onLogout}
      syncing={syncing}
      searchQuery={searchQuery}
      onSearchChange={setSearchQuery}
      statusFilters={statusFilters}
      onFilterChange={setStatusFilters}
      runtimeFilter={runtimeFilter}
      onRuntimeFilterChange={setRuntimeFilter}
      totalCount={totalSessionCount}
      filteredCount={matchedSessionCount}
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

      <Suspense fallback={<SessionCardSkeleton count={4} />}>
        {activeTab === "sessions" && !loading && (
          <>
            {selectedProjectRoot && (
              <div className={layoutStyles.projectFilterBar}>
                <span className={layoutStyles.projectFilterText}>
                  Project:
                  <strong>
                    {selectedProject?.name ||
                      selectedProjectRoot.split("/").pop() ||
                      selectedProjectRoot}
                  </strong>
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
              hasActiveFilters={hasActiveFilters}
              totalCount={totalSessionCount}
              globalMatchCount={matchedSessionCount}
              initiallyExpandedSessionId={expandedSessionId ?? undefined}
              onInitialExpansionConsumed={handleInitialExpansionConsumed}
            />
          </>
        )}

        {activeTab === "analytics" && !loading && <UsagePanel />}

        {activeTab === "orchestrator" && !loading && (
          <OrchestratorPanel
            token={token}
            onOpenSession={handleOpenOrchestratorSession}
            onRecover={handleRecover}
            onStop={handleStop}
            onComplete={handleComplete}
            onCopyRecoveryCommand={handleCopyRecoveryCommand}
          />
        )}

        {activeTab === "work" && !loading && <WorkItemsPanel token={token} />}

        {activeTab === "projects" && !loading && (
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

        {activeTab === "memory" && !loading && <MemoryPanel />}

        {activeTab === "plans" && !loading && <PlansPanel />}
      </Suspense>

      <HelpModal isOpen={showHelp} onClose={() => setShowHelp(false)} />
    </Layout>
  );
}
