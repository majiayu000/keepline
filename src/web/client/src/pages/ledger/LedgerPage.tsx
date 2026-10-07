import { useCallback, useEffect, useMemo, useState } from "react";
import type { TabId } from "@/components/TabNav";
import { useLedger } from "@/hooks/useLedger";
import type { LedgerProps } from "./types";
import type { LedgerDetail } from "../../../../../domain/ledger/types";
import { groupRows, projectName, rowPresentation } from "./presentation";
import { WorkspaceShell } from "./components/WorkspaceShell";
import { LedgerOverview } from "./components/LedgerOverview";
import {
  LedgerTodos,
  LedgerGoals,
  LedgerReview,
} from "./components/LedgerWorkViews";
import { LedgerSettings } from "./components/LedgerSettings";
import { LedgerDrawer } from "./components/LedgerDrawer";
import { LedgerAgentPanel } from "./components/LedgerAgentPanel";
import { LedgerDialogs } from "./components/LedgerDialogs";
import { ActionButton } from "./components/LedgerPrimitives";
import styles from "./Workspace.module.css";

export function LedgerPage({
  token,
  view,
  onOpenSession,
  onNavigate,
  onLogout,
  onStop,
}: LedgerProps & {
  onNavigate: (view: TabId) => void;
  onLogout: () => Promise<void>;
  onStop: (id: string) => Promise<void>;
}) {
  const c = useLedger({ token, view, onOpenSession });
  const [search, setSearch] = useState("");
  const [project, setProject] = useState<string | null>(null);
  const [full, setFull] = useState(
    () =>
      new URLSearchParams(window.location.search).get("detail") === "full" ||
      new URLSearchParams(window.location.search).has("anchor"),
  );
  const [cursor, setCursor] = useState(0);
  const [keyboardCursor, setKeyboardCursor] = useState(false);
  const filtered = useMemo(
    () =>
      c.rows.filter(
        (r) =>
          (!project || r.projectRoot === project) &&
          `${r.title} ${r.projectRoot} ${r.runtimeId} ${r.asks.map((a) => a.text).join(" ")}`
            .toLowerCase()
            .includes(search.toLowerCase()),
      ),
    [c.rows, project, search],
  );
  const ordered = groupRows(filtered, "urgency").flatMap((g) => g.rows);
  const projects = useMemo(
    () =>
      [...new Set(c.rows.map((r) => r.projectRoot))].map((root) => ({
        root,
        name: projectName(root),
        count: c.rows.filter(
          (r) => r.projectRoot === root && !rowPresentation(r).ended,
        ).length,
        need: c.rows.filter(
          (r) => r.projectRoot === root && rowPresentation(r).need,
        ).length,
      })),
    [c.rows],
  );
  const pendingCount = (c.queuedRows ?? []).filter((next) => {
    const old = c.rows.find((r) => r.sessionId === next.sessionId);
    return (
      !old ||
      old.lastActiveAt !== next.lastActiveAt ||
      old.state !== next.state ||
      old.title !== next.title ||
      old.progress.done !== next.progress.done ||
      old.progress.total !== next.progress.total
    );
  }).length;
  const counts = {
    need: filtered.filter((r) => rowPresentation(r).need).length,
    run: filtered.filter((r) => {
      const p = rowPresentation(r);
      return !p.need && !p.paused && !p.ended;
    }).length,
    paused: filtered.filter((r) => rowPresentation(r).paused).length,
    ended: filtered.filter((r) => rowPresentation(r).ended).length,
  };
  const allTodos = c.todos.filter((t) => t.status !== "archived");
  const subtitles = {
    overview: `最近 ${c.hours} 小时 · ${counts.need} 件事需要你，${counts.run} 个在推进，${counts.paused} 个暂停，${counts.ended} 个已结束${project ? ` · ${projectName(project)}` : ""}`,
    todos: `${allTodos.filter((t) => t.status !== "done").length} 个未完成 · ${allTodos.filter((t) => !c.rows.some((r) => r.workItemId === t.id)).length} 个还没派出`,
    goals: `${c.goals.length} 个目标，本周推进 ${c.goals.reduce((n, g) => n + g.weeklyMovement, 0)} 个待办`,
    review: "按天回看：还挂着、已验收、偏离和纠正",
    "ledger-settings": "每个开关都写明后果",
  };
  const select = useCallback(
    (id: string) => {
      c.setSelectedId(id);
      c.setItemFilter(null);
    },
    [c.setSelectedId, c.setItemFilter],
  );
  const navigate = useCallback(
    (tab: TabId) => {
      c.setSelectedId(null);
      setFull(false);
      onNavigate(tab);
    },
    [c.setSelectedId, onNavigate],
  );
  const openFull = () => {
    setFull(true);
    const url = new URL(window.location.href);
    url.searchParams.set("detail", "full");
    window.history.replaceState(null, "", url);
  };
  useEffect(() => {
    const url = new URL(window.location.href);
    if (full && c.selectedId) url.searchParams.set("detail", "full");
    else url.searchParams.delete("detail");
    window.history.replaceState(null, "", url);
  }, [full, c.selectedId]);
  const closeFull = () => {
    setFull(false);
    c.setSelectedId(null);
    const url = new URL(window.location.href);
    url.searchParams.delete("detail");
    window.history.replaceState(null, "", url);
  };
  const operate = (row: LedgerDetail, action: string) => {
    if (action === "跳到 agent") {
      onOpenSession(row.sessionId);
      return;
    }
    if (action === "停止") {
      void c.perform(() => onStop(row.sessionId));
      return;
    }
    if (action === "看进度账") {
      select(row.sessionId);
      openFull();
      return;
    }
    if (action === "复制追问") {
      void c.perform(() => c.openPrompt(row));
      return;
    }
    if (action === "验收通过") {
      void c.perform(() => c.accept(row, "accepted"));
      return;
    }
    const reason = window.prompt("为什么放弃剩余要求？");
    if (reason?.trim())
      void c.perform(() => c.accept(row, "accepted_with_gaps", reason));
  };
  const primaryAction = (row: LedgerDetail) => {
    if (row.state === "needs_input") {
      onOpenSession(row.sessionId);
      return;
    }
    select(row.sessionId);
    if (row.state === "review" && row.progress.total === 0) return;
    if (row.offPlan.length) {
      openFull();
      return;
    }
    if (row.state === "review" && row.progress.done < row.progress.total)
      void c.perform(() => c.openPrompt(row));
    else if (row.state === "review")
      void c.perform(() => c.accept(row, "accepted"));
    else openFull();
  };
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (
        (e.target as HTMLElement).closest(
          'input,textarea,select,[contenteditable="true"]',
        ) ||
        c.prompt ||
        c.editing ||
        c.itemEditor
      )
        return;
      if (e.key === "Escape") {
        c.setSelectedId(null);
        setFull(false);
      }
      if (
        full &&
        (e.key.toLowerCase() === "j" || e.key.toLowerCase() === "k")
      ) {
        e.preventDefault();
        const index = ordered.findIndex((r) => r.sessionId === c.selectedId);
        const next =
          ordered[
            Math.max(
              0,
              Math.min(
                index + (e.key.toLowerCase() === "j" ? 1 : -1),
                ordered.length - 1,
              ),
            )
          ];
        if (next) select(next.sessionId);
        return;
      }
      const row = c.detail ?? ordered[Math.min(cursor, ordered.length - 1)];
      if (view === "overview") {
        if (e.key.toLowerCase() === "j") {
          e.preventDefault();
          setKeyboardCursor(true);
          setCursor((i) => Math.max(0, Math.min(i + 1, ordered.length - 1)));
        }
        if (e.key.toLowerCase() === "k") {
          e.preventDefault();
          setKeyboardCursor(true);
          setCursor((i) => Math.max(i - 1, 0));
        }
        if (
          e.key === "Enter" &&
          row &&
          !(e.target as HTMLElement).closest('button,[role="button"]')
        )
          select(row.sessionId);
      }
      if (!row) return;
      if (e.key.toLowerCase() === "o") onOpenSession(row.sessionId);
      if (e.key.toLowerCase() === "c") void c.perform(() => c.openPrompt(row));
      if (e.key.toLowerCase() === "a" && row.state === "review")
        primaryAction(row);
    };
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  });
  const dialogs = (
    <LedgerDialogs controller={c} onOpenSession={onOpenSession} />
  );
  if (full && c.detail)
    return (
      <>
        <LedgerAgentPanel
          controller={c}
          onBack={closeFull}
          onOpenSession={onOpenSession}
        />
        {dialogs}
      </>
    );
  return (
    <WorkspaceShell
      view={view}
      onNavigate={navigate}
      subtitle={subtitles[view]}
      search={search}
      onSearch={setSearch}
      projects={projects}
      project={project}
      onProject={(root) => {
        setProject(root);
        setCursor(0);
        navigate("overview");
      }}
      needCount={c.rows.filter((r) => rowPresentation(r).need).length}
      todoCount={allTodos.filter((t) => t.status !== "done").length}
      focusUntil={c.settings.focus.until}
      focusMinutes={c.settings.focus.minutes}
      onFocus={() => void c.perform(c.toggleFocus)}
      onLogout={() => void onLogout()}
      onRefresh={() => void c.perform()}
      syncStatus={c.syncStatus}
    >
      {c.error && (
        <div className={styles.errorBanner} role="alert">
          {c.error}
          <ActionButton onClick={() => void c.perform()}>
            重试
          </ActionButton>
        </div>
      )}
      {c.loading && !c.error && (
        <div className={styles.pageContent} role="status">
          正在读取执行记录…
        </div>
      )}
      {view === "overview" && pendingCount > 0 && (
        <div className={styles.pendingUpdate}>
          鼠标在面板内，已暂停重排 · 有 {pendingCount} 条更新
        </div>
      )}
      {!c.loading && view === "overview" && (
        <LedgerOverview
          rows={filtered}
          hours={c.hours}
          maxHours={c.settings.retentionDays * 24}
          onHoursChange={c.changeHours}
          selectedId={c.selectedId}
          cursor={keyboardCursor ? ordered[cursor]?.sessionId : undefined}
          onSelect={select}
          onAction={primaryAction}
          onMenu={operate}
          onPause={c.pauseRows}
          onResume={c.resumeRows}
        />
      )}
      {!c.loading && view === "todos" && (
        <LedgerTodos controller={c} onSelect={select} />
      )}
      {!c.loading && view === "goals" && (
        <LedgerGoals controller={c} onTodos={() => navigate("todos")} onSelect={select} onOpenSession={onOpenSession} />
      )}
      {!c.loading && view === "review" && (
        <LedgerReview
          controller={c}
          onSelect={select}
          onOpenSession={onOpenSession}
        />
      )}
      {!c.loading && view === "ledger-settings" && (
        <LedgerSettings controller={c} />
      )}
      {c.selectedId && !c.detail && !c.error && (
        <div className={styles.drawer} role="status">
          正在读取会话详情…
        </div>
      )}
      {c.detail && (
        <LedgerDrawer
          row={c.detail}
          onClose={() => c.setSelectedId(null)}
          onOpenSession={() => onOpenSession(c.detail!.sessionId)}
          onFullDetail={openFull}
          recovery={c.recovery}
          onCopyRecovery={() =>
            void c.perform(() =>
              navigator.clipboard.writeText(c.recovery!.command!),
            )
          }
        />
      )}
      {dialogs}
    </WorkspaceShell>
  );
}
