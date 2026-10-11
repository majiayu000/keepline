import { useCallback, useState, useRef, useEffect } from "react";
import { useToast } from "@/components/Toast";
import {
  useSessions,
  useKeyboardShortcuts,
  useNotifications,
  useProjects,
} from "@/hooks";
import { fetchSession } from "@/services/api";
import type { ProjectInfo, RuntimeFilter, SessionStatus } from "@/types";
import type { TabId } from "@/components/TabNav";
export interface DashboardProps {
  activeTab: TabId;
  onNavigate: (view: TabId) => void;
  openSessionId: string | null;
  token: string;
  onLogout: () => Promise<void>;
}

export function useDashboard({
  token,
  onNavigate: setActiveTab,
  activeTab,
  openSessionId,
}: DashboardProps) {
  const { showToast } = useToast();
  const [showHelp, setShowHelp] = useState(false);
  const [selectedProjectRoot, setSelectedProjectRoot] = useState<string | null>(
    null,
  );
  const [searchQuery, setSearchQuery] = useState(openSessionId ?? "");
  const [statusFilters, setStatusFilters] = useState<Set<SessionStatus>>(
    new Set(),
  );
  const [runtimeFilter, setRuntimeFilter] = useState<RuntimeFilter>("all");
  const [expandedSessionId, setExpandedSessionId] = useState<string | null>(
    openSessionId,
  );

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
    getSessionFull,
    loadSessionFull,
    isLoadingFull,
    pagination,
    loadMore,
    loadingMore,
    connectionStatus,
    version: sessionsVersion,
  } = useSessions(
    token,
    activeTab === "sessions"
      ? {
          searchQuery,
          statusFilters,
          runtimeFilter,
          projectRoot: selectedProjectRoot ?? undefined,
        }
      : {},
  );

  const filteredSessions = sessions;
  const totalSessionCount =
    allSessions.length > 0
      ? allSessions.length
      : (stats?.total ?? sessions.length);
  const matchedSessionCount = pagination?.total ?? sessions.length;
  const hasActiveFilters =
    searchQuery.trim().length > 0 ||
    statusFilters.size > 0 ||
    runtimeFilter !== "all" ||
    Boolean(selectedProjectRoot);

  const {
    projects,
    stats: projectStats,
    loading: projectsLoading,
    error: projectsError,
  } = useProjects(token, sessionsVersion);

  const selectedProject = selectedProjectRoot
    ? projects.find((project) => project.rootPath === selectedProjectRoot)
    : undefined;

  const {
    settings: notificationSettings,
    updateSettings: updateNotificationSettings,
    permission: notificationPermission,
    requestPermission: requestNotificationPermission,
    checkSessionChanges,
  } = useNotifications();

  const prevSessionsRef = useRef<typeof sessions>([]);
  useEffect(() => {
    const notificationSessions =
      allSessions.length > 0 ? allSessions : sessions;
    if (notificationSessions.length > 0 && prevSessionsRef.current.length > 0) {
      checkSessionChanges(prevSessionsRef.current, notificationSessions);
    }
    prevSessionsRef.current = notificationSessions;
  }, [allSessions, sessions, checkSessionChanges]);

  const handleSync = useCallback(async () => {
    const success = await sync();
    showToast(
      success ? "Sync completed" : "Sync failed",
      success ? "success" : "error",
    );
  }, [sync, showToast]);

  const handleRecover = useCallback(
    async (sessionId: string, terminalApp?: import("@/types").TerminalApp) => {
      const result = await recoverSession(sessionId, terminalApp);
      showToast(
        result.success
          ? `Session opened in ${terminalApp || "terminal"}`
          : result.error || "Failed to recover session",
        result.success ? "success" : "error",
      );
    },
    [recoverSession, showToast],
  );

  const handleStop = useCallback(
    async (sessionId: string) => {
      const success = await stopSession(sessionId);
      showToast(
        success ? "Session stopped" : "Failed to stop session",
        success ? "success" : "error",
      );
    },
    [stopSession, showToast],
  );

  const handleComplete = useCallback(
    async (sessionId: string) => {
      const success = await completeSession(sessionId);
      showToast(
        success ? "Session marked as completed" : "Failed to complete session",
        success ? "success" : "error",
      );
    },
    [completeSession, showToast],
  );

  const handleProjectClick = useCallback(
    (project: ProjectInfo) => {
      setSelectedProjectRoot(project.rootPath);
      setSearchQuery("");
      setActiveTab("sessions");
      showToast(`Filtered to: ${project.name}`, "info");
    },
    [showToast],
  );

  const handleClearProjectFilter = useCallback(() => {
    setSelectedProjectRoot(null);
    showToast("Project filter cleared", "info");
  }, [showToast]);

  const handleOpenOrchestratorSession = useCallback((sessionId: string) => {
    setSelectedProjectRoot(null);
    setStatusFilters(new Set());
    setRuntimeFilter("all");
    setSearchQuery(sessionId);
    setExpandedSessionId(sessionId);
    setActiveTab("sessions");
  }, []);

  const handleInitialExpansionConsumed = useCallback(() => {
    setExpandedSessionId(null);
  }, []);

  const handleCopyRecoveryCommand = useCallback(
    async (sessionId: string) => {
      try {
        const response = await fetchSession(sessionId);
        const command = response.data?.recovery.command;
        if (!response.success || !command) {
          showToast(
            response.error || "Recovery command is not available",
            "error",
          );
          return;
        }

        await navigator.clipboard.writeText(command);
        showToast("Recovery command copied", "success");
      } catch {
        showToast("Failed to copy recovery command", "error");
      }
    },
    [showToast],
  );

  useKeyboardShortcuts({
    onRefresh: refresh,
    onSync: handleSync,
    onShowHelp: () => setShowHelp(true),
  });

  return {
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
  };
}
