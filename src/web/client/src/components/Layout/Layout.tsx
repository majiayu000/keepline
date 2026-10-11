import { ReactNode, memo } from "react";
import { Header } from "@/components/Header";
import { WorkspaceShell } from "@/pages/ledger/components/WorkspaceShell";
import { StatsBar } from "@/components/StatsBar";
import { Toolbar } from "@/components/Toolbar";
import { type TabId } from "@/components/TabNav";
import type {
  RuntimeFilter,
  Session,
  SessionStats,
  SessionStatus,
} from "@/types";
import type { NotificationSettings, ConnectionStatus } from "@/hooks";
import styles from "./Layout.module.css";

interface LayoutProps {
  children: ReactNode;
  stats: SessionStats | null;
  loading?: boolean;
  onSync: () => void;
  onLogout?: () => void | Promise<void>;
  syncing?: boolean;
  // Search & Filter props
  searchQuery?: string;
  onSearchChange?: (query: string) => void;
  statusFilters?: Set<SessionStatus>;
  onFilterChange?: (filters: Set<SessionStatus>) => void;
  runtimeFilter?: RuntimeFilter;
  onRuntimeFilterChange?: (filter: RuntimeFilter) => void;
  totalCount?: number;
  filteredCount?: number;
  // Export props
  sessions?: Session[];
  // Notification props
  notificationSettings?: NotificationSettings;
  onUpdateNotificationSettings?: (
    updates: Partial<NotificationSettings>,
  ) => void;
  notificationPermission?: NotificationPermission;
  onRequestNotificationPermission?: () => Promise<boolean>;
  // Connection status
  connectionStatus?: ConnectionStatus;
  // Tab navigation
  activeTab?: TabId;
  onTabChange?: (tab: TabId) => void;
}

export const Layout = memo(function Layout({
  children,
  stats,
  loading,
  onSync,
  onLogout,
  syncing,
  searchQuery = "",
  onSearchChange,
  statusFilters = new Set(),
  onFilterChange,
  runtimeFilter = "all",
  onRuntimeFilterChange,
  totalCount = 0,
  filteredCount = 0,
  sessions = [],
  notificationSettings,
  onUpdateNotificationSettings,
  notificationPermission,
  onRequestNotificationPermission,
  connectionStatus,
  activeTab = "sessions",
  onTabChange,
}: LayoutProps) {
  const showToolbar = onSearchChange && onFilterChange && onRuntimeFilterChange;

  return (
    <WorkspaceShell
      view={activeTab}
      onNavigate={(tab) => onTabChange?.(tab)}
      subtitle={
        connectionStatus === "realtime"
          ? "实时连接 · 本地会话记录"
          : connectionStatus === "disconnected"
            ? "连接已断开"
            : "定时刷新 · 本地会话记录"
      }
      projects={[...new Set(sessions.map((s) => s.directory))].map((root) => ({
        root,
        name: root.split("/").filter(Boolean).at(-1) || root,
        count: sessions.filter(
          (s) => s.directory === root && s.status !== "completed",
        ).length,
        need: sessions.filter(
          (s) => s.directory === root && s.status === "needs_input",
        ).length,
      }))}
      needCount={stats?.needs_input}
      onProject={(root) => {
        onSearchChange?.(root ?? "");
        onTabChange?.("sessions");
      }}
      onRefresh={onSync}
      onLogout={() => void onLogout?.()}
    >
      <Header
        onSync={onSync}
        onLogout={onLogout}
        syncing={syncing}
        sessions={sessions}
        notificationSettings={notificationSettings}
        onUpdateNotificationSettings={onUpdateNotificationSettings}
        notificationPermission={notificationPermission}
        onRequestNotificationPermission={onRequestNotificationPermission}
        connectionStatus={connectionStatus}
      />
      {activeTab === "sessions" && (
        <>
          <StatsBar stats={stats} loading={loading} />
          {showToolbar && (
            <Toolbar
              searchQuery={searchQuery}
              onSearchChange={onSearchChange}
              statusFilters={statusFilters}
              onFilterChange={onFilterChange}
              runtimeFilter={runtimeFilter}
              onRuntimeFilterChange={onRuntimeFilterChange}
              stats={stats}
              totalCount={totalCount}
              filteredCount={filteredCount}
            />
          )}
        </>
      )}
      <main className={styles.main}>{children}</main>
    </WorkspaceShell>
  );
});

// Export styles for use in App.tsx
export { styles as layoutStyles };
