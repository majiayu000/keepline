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
  const [route, setRoute] = useState(() => new URL(window.location.href));
  const requestedView = route.searchParams.get("view");
  const view: TabId = views.includes(requestedView as TabId) ? requestedView as TabId : "overview";
  const selectedId = route.searchParams.get("sessionId");
  const fullDetail = route.searchParams.get("detail") === "full" || route.searchParams.has("anchor");
  const commitRoute = useCallback((url: URL, replace = false, detail = false) => {
    if (url.href === window.location.href) return;
    const state = detail ? {
      keeplineBase: window.history.state?.keeplineBase ?? window.location.href,
      keeplineDepth: (window.history.state?.keeplineDepth ?? 0) + 1,
    } : null;
    if (replace) window.history.replaceState(state, "", url);
    else window.history.pushState(state, "", url);
    setRoute(url);
  }, []);
  useEffect(() => {
    const back = () => setRoute(new URL(window.location.href));
    window.addEventListener("popstate", back);
    return () => window.removeEventListener("popstate", back);
  }, []);
  const navigate = useCallback((next: TabId) => {
    const url = new URL(window.location.href);
    url.searchParams.set("view", next);
    for (const key of ["sessionId", "detail", "anchor"]) url.searchParams.delete(key);
    commitRoute(url);
  }, [commitRoute]);
  const openSession = useCallback((id: string) => {
    const url = new URL(window.location.href);
    url.searchParams.set("view", "sessions");
    url.searchParams.set("sessionId", id);
    url.searchParams.delete("detail");
    url.searchParams.delete("anchor");
    commitRoute(url);
  }, [commitRoute]);
  const selectSession = useCallback((id: string | null, full?: boolean) => {
    const url = new URL(window.location.href);
    if (id) url.searchParams.set("sessionId", id);
    else url.searchParams.delete("sessionId");
    if (id && (full ?? (url.searchParams.get("detail") === "full" || url.searchParams.has("anchor")))) url.searchParams.set("detail", "full");
    else url.searchParams.delete("detail");
    url.searchParams.delete("anchor");
    commitRoute(url, false, !!id);
  }, [commitRoute]);
  const closeDetail = useCallback(() => {
    const url = new URL(window.location.href);
    for (const key of ["sessionId", "detail", "anchor"]) url.searchParams.delete(key);
    const state = window.history.state;
    if (state?.keeplineDepth > 0 && state.keeplineBase === url.href) window.history.go(-state.keeplineDepth);
    else commitRoute(url, true);
  }, [commitRoute]);
  const changeScope = useCallback((key: "q" | "project", value: string | null) => {
    const url = new URL(window.location.href);
    if (value) url.searchParams.set(key, value);
    else url.searchParams.delete(key);
    for (const name of ["sessionId", "detail", "anchor"]) url.searchParams.delete(name);
    commitRoute(url, key === "q");
  }, [commitRoute]);
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
          selectedId={selectedId}
          onSelectSession={selectSession}
          onCloseDetail={closeDetail}
          fullDetail={fullDetail}
          search={route.searchParams.get("q") ?? ""}
          project={route.searchParams.get("project")}
          onSearch={(value) => changeScope("q", value)}
          onProject={(value) => changeScope("project", value)}
          onLogout={auth.logout}
          onStop={stop}
        />
      ) : (
        <DashboardPage
          token={token}
          activeTab={view}
          onNavigate={navigate}
          onLogout={auth.logout}
          openSessionId={view === "sessions" ? selectedId : null}
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
