import { lazy, Suspense, useCallback, useEffect, useState } from "react";
import { ToastProvider, useToast } from "@/components/Toast";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { SessionCardSkeleton } from "@/components/Skeleton";
import { AuthSetup } from "@/components/AuthSetup";
import { AuthLogin } from "@/components/AuthLogin";
import { MenubarPage } from "@/pages/ledger/MenubarPage";
import { useAuth } from "@/hooks/useAuth";
import { stopSession } from "@/services/api";
import type { TabId } from "@/components/TabNav";

const LedgerPage = lazy(() =>
  import("@/pages/ledger/LedgerPage").then((m) => ({ default: m.LedgerPage })),
);
const DashboardPage = lazy(() =>
  import("@/pages/DashboardPage").then((m) => ({ default: m.DashboardPage })),
);
const views: TabId[] = [
  "overview",
  "todos",
  "goals",
  "review",
  "ledger-settings",
  "sessions",
  "projects",
  "analytics",
  "orchestrator",
  "work",
  "plans",
  "memory",
];
function AppContent() {
  const auth = useAuth();
  const { showToast } = useToast();
  const token = auth.getToken();
  const [view, setView] = useState<TabId>(() => {
    const value = new URLSearchParams(window.location.search).get("view");
    return views.includes(value as TabId) ? (value as TabId) : "overview";
  });
  const [openSessionId, setOpenSessionId] = useState<string | null>(null);
  useEffect(() => {
    const url = new URL(window.location.href);
    url.searchParams.set("view", view);
    window.history.replaceState(null, "", url);
  }, [view]);
  const navigate = useCallback((next: TabId) => {
    setView(next);
    setOpenSessionId(null);
  }, []);
  const openSession = useCallback((id: string) => {
    setOpenSessionId(id);
    setView("sessions");
  }, []);
  const stop = useCallback(
    async (id: string) => {
      const result = await stopSession(id);
      if (!result.success) throw new Error(result.error || "停止会话失败");
      showToast("会话已停止", "success");
    },
    [showToast],
  );
  if (auth.loading) return <SessionCardSkeleton count={4} />;
  if (!auth.status)
    return (
      <div role="alert" style={{ padding: 24 }}>
        {auth.error || "无法读取登录状态"}{" "}
        <button onClick={() => void auth.checkStatus()}>重试</button>
      </div>
    );
  if (!auth.status.setupComplete)
    return <AuthSetup onSetup={auth.setup} error={auth.error} />;
  if (!auth.status.authenticated || !token)
    return (
      <AuthLogin
        onLogin={auth.login}
        onLocalLogin={auth.localLogin}
        error={auth.error}
      />
    );
  if (window.location.pathname === "/menubar")
    return <MenubarPage token={token} />;
  return (
    <Suspense fallback={<SessionCardSkeleton count={4} />}>
      {view === "overview" ||
      view === "todos" ||
      view === "goals" ||
      view === "review" ||
      view === "ledger-settings" ? (
        <LedgerPage
          token={token}
          view={view}
          onNavigate={navigate}
          onOpenSession={openSession}
          onLogout={auth.logout}
          onStop={stop}
        />
      ) : (
        <DashboardPage
          token={token}
          activeTab={view}
          onNavigate={navigate}
          onLogout={auth.logout}
          openSessionId={openSessionId}
        />
      )}
    </Suspense>
  );
}
export default function App() {
  return (
    <ErrorBoundary>
      <ToastProvider>
        <AppContent />
      </ToastProvider>
    </ErrorBoundary>
  );
}
