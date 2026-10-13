import { useCallback, useEffect, useMemo, useState } from "react";
import type { TabId } from "@/components/TabNav";
import { useLedger } from "@/hooks/useLedger";
import type { WorkItem } from "@/types/work-item";
import type { LedgerProps } from "./types";
import type { LedgerDetail } from "../../../../../domain/ledger/types";
import { groupRows, projectName, rowPresentation, attentionSummary, monitorPresentation } from "./presentation";
import { WorkspaceShell } from "./components/WorkspaceShell";
import { LedgerOverview, savedOverviewView } from "./components/LedgerOverview";
import {
  LedgerTodos,
  LedgerGoals,
  LedgerReview,
} from "./components/LedgerWorkViews";
import { LedgerSettings } from "./components/LedgerSettings";
import { LedgerDrawer } from "./components/LedgerDrawer";
import { LedgerOverviewSkeleton, LedgerDrawerSkeleton } from "./components/LedgerSkeleton";
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
  selectedId,
  onSelectSession,
  onCloseDetail,
  fullDetail: full,
  search,
  project,
  onSearch,
  onProject,
}: LedgerProps & {
  onNavigate: (view: TabId) => void;
  onLogout: () => Promise<void>;
  onStop: (id: string) => Promise<void>;
  onCloseDetail: () => void;
  fullDetail: boolean;
  search: string;
  project: string | null;
  onSearch: (value: string) => void;
  onProject: (value: string | null) => void;
}) {
  const c = useLedger({ token, view, onOpenSession, selectedId, onSelectSession, project });
  const [narrow, setNarrow] = useState(() => window.matchMedia("(max-width: 700px)").matches);
  const [stopRow, setStopRow] = useState<LedgerDetail | null>(null);
  useEffect(() => {
    const media = window.matchMedia("(max-width: 700px)");
    const update = () => setNarrow(media.matches);
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  useEffect(() => {
    if (!selectedId || full) return;
    const previous = document.activeElement as HTMLElement | null;
    const before = document.body.style.overflow;
    if (narrow) document.body.style.overflow = "hidden";
    document.querySelector<HTMLElement>('aside[role="dialog"] button[aria-label="关闭"]')?.focus();
    const trap = (event: KeyboardEvent) => {
      if (!narrow || event.key !== "Tab") return;
      const drawer = document.querySelector<HTMLElement>('aside[role="dialog"]');
      if (!drawer) return;
      const controls = [...drawer.querySelectorAll<HTMLElement>('button:not(:disabled),a[href],input:not(:disabled),textarea:not(:disabled),select:not(:disabled),[tabindex="0"]')]
        .filter(control => control.getClientRects().length > 0);
      const first = controls[0], last = controls.at(-1);
      if (!drawer.contains(document.activeElement) || event.shiftKey && document.activeElement === first || !event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        (event.shiftKey ? last : first)?.focus();
      }
    };
    document.addEventListener("keydown", trap);
    return () => {
      document.removeEventListener("keydown", trap);
      document.body.style.overflow = before;
      if (previous?.isConnected) previous.focus();
    };
  }, [selectedId, full, narrow]);
  const [grouping, setGrouping] = useState<"recent" | "urgency" | "project" | "goal">(() => {
    const saved = savedOverviewView()?.grouping;
    return saved === "goal" || saved === "project" || saved === "urgency" ? saved : "recent";
  });
  const [heldGroups, setHeldGroups] = useState<ReturnType<typeof groupRows> | null>(null);
  const [cursor, setCursor] = useState<string>();
  const [allOutside, setAllOutside] = useState(false);
  useEffect(() => setAllOutside(false), [c.hours, project, search]);
  const filtered = useMemo(
    () =>
      c.rows.filter(
        (r) =>
          (!project || r.projectRoot === project) &&
          `${r.title} ${r.projectRoot} ${r.runtimeId} ${r.asks.map((a) => a.text).join(" ")}`
            .toLowerCase()
            .includes(search.trim().toLowerCase()),
      ),
    [c.rows, project, search],
  );
  const freshGroups = useMemo(() => groupRows(filtered, grouping, Date.now(), { goals: c.goals, todos: c.todos }), [filtered, grouping, c.goals, c.todos]);
  const groups = useMemo(() => {
    if (!heldGroups) return freshGroups;
    const current = new Map(filtered.map(row => [row.sessionId, row]));
    const heldIds = new Set(heldGroups.flatMap(group => group.rows.map(row => row.sessionId)));
    const heldKeys = new Set(heldGroups.map(group => group.key));
    return [
      ...heldGroups.map(group => ({
        ...group,
        rows: [
          ...group.rows.flatMap(row => current.has(row.sessionId) ? [current.get(row.sessionId)!] : []),
          ...(freshGroups.find(next => next.key === group.key)?.rows ?? []).filter(row => !heldIds.has(row.sessionId)),
        ],
      })),
      ...freshGroups.filter(group => !heldKeys.has(group.key)).map(group => ({ ...group, rows: group.rows.filter(row => !heldIds.has(row.sessionId)) })),
    ];
  }, [filtered, freshGroups, heldGroups]);
  const ordered = (full ? groupRows(c.rows, "urgency") : groups).flatMap(group => group.rows);
  useEffect(() => {
    setHeldGroups(null);
    setCursor(undefined);
  }, [grouping, project, search, c.hours, view, full]);
  useEffect(() => {
    if (!cursor) return;
    const card = [...document.querySelectorAll<HTMLElement>("[data-session-id]")]
      .find(element => element.dataset.sessionId === cursor);
    card?.focus({ preventScroll: true });
    card?.scrollIntoView({ block: "nearest" });
  }, [cursor]);
  const outside = c.outsideAttention.filter(row => (!project || row.projectRoot === project) && `${row.title} ${row.projectRoot}`.toLowerCase().includes(search.toLowerCase()));
  const projects = useMemo(
    () =>
      [...new Set([...c.rows.map((r) => r.projectRoot), ...c.goals.map(goal => goal.projectRoot), ...c.todos.map(todo => todo.projectRoot)].filter((root): root is string => !!root))].map((root) => ({
        root,
        name: projectName(root),
        hasWork: c.todos.some(todo => todo.projectRoot === root && todo.status !== "done" && todo.status !== "archived"),
        count: c.rows.filter(
          (r) => r.projectRoot === root && rowPresentation(r).online && !rowPresentation(r).historical,
        ).length,
        need: c.rows.filter(
          (r) => r.projectRoot === root && rowPresentation(r).need,
        ).length,
      })),
    [c.rows, c.goals, c.todos],
  );
  const pendingCount = heldGroups ? freshGroups.flatMap(group => group.rows)
    .filter((row, index) => ordered[index]?.sessionId !== row.sessionId).length : 0;
  const counts = {
    online: filtered.filter((r) => rowPresentation(r).online).length,
    exited: filtered.filter((r) => rowPresentation(r).exited).length,
    unknown: filtered.filter((r) => rowPresentation(r).unknown).length,
  };
  const query = search.trim().toLowerCase();
  const matchesWork = (item: WorkItem, parent?: WorkItem) =>
    item.status !== "archived" && (!project || (item.projectRoot ?? parent?.projectRoot) === project) &&
    `${item.title} ${item.body ?? ""} ${item.outcome ?? ""} ${item.projectRoot ?? ""} ${parent?.title ?? ""} ${(item.acceptance ?? []).map(check => check.text).join(" ")}`.toLowerCase().includes(query);
  const scopedTodos = c.todos.filter(todo => matchesWork(todo, c.goals.find(goal => goal.id === todo.parentId)));
  const scopedGoals = c.goals.map(goal => ({ ...goal, todos: goal.todos.filter(todo => matchesWork(todo, goal)) }))
    .filter(goal => goal.todos.length > 0 || matchesWork(goal));
  const scopedController = { ...c, rows: filtered, todos: scopedTodos, goals: scopedGoals };
  const allTodos = scopedTodos;
  const attention = !c.monitor || c.monitorError || monitorPresentation(c.monitor).warning
    ? "待处理数量尚未确认" : attentionSummary(filtered);
  const subtitles = {
    overview: `最近 ${c.hours} 小时 · ${attention} · ${counts.online} 个在线，${counts.exited} 个已退出，${counts.unknown} 个在线状态待确认 · ${filtered.length} 条会话记录${project ? ` · ${projectName(project)}` : ""}`,
    todos: `${allTodos.filter((t) => t.status !== "done").length} 个未完成 · ${allTodos.filter((t) => !c.rows.some((r) => r.workItemId === t.id)).length} 个还没派出`,
    goals: `${scopedGoals.length} 个目标，本周推进 ${scopedGoals.reduce((n, g) => n + g.weeklyMovement, 0)} 个待办`,
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
  const navigate = onNavigate;
  const openFull = (id = c.selectedId) => {
    if (!id) return;
    c.setItemFilter(null);
    onSelectSession(id, true);
  };
  const closeFull = onCloseDetail;
  const operate = (row: LedgerDetail, action: string) => {
    if (action === "查看会话记录") {
      onOpenSession(row.sessionId);
      return;
    }
    if (action === "停止进程") {
      setStopRow(row);
      return;
    }
    if (action === "看进度账") {
      openFull(row.sessionId);
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
    if (row.state === "review" && row.progress.total === 0) { select(row.sessionId); return; }
    if (row.offPlan.length) {
      openFull(row.sessionId);
      return;
    }
    if (row.state === "review") {
      select(row.sessionId);
      if (row.progress.done < row.progress.total) void c.perform(() => c.openPrompt(row));
      else void c.perform(() => c.accept(row, "accepted"));
    } else openFull(row.sessionId);
  };
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (
        e.defaultPrevented || e.isComposing || e.metaKey || e.ctrlKey || e.altKey || e.repeat ||
        (e.target as HTMLElement).closest(
          'input,textarea,select,[contenteditable="true"],[role="menu"]',
        ) ||
        c.prompt ||
        c.editing ||
        c.itemEditor || stopRow
      )
        return;
      if (e.key === "Escape") {
        onCloseDetail();
        return;
      }
      if (
        (full || view === "overview" && c.selectedId) &&
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
      const target = e.target as HTMLElement;
      const focusedId = target.closest<HTMLElement>("[data-session-id]")?.dataset.sessionId;
      const row = c.detail ?? ordered.find(row => row.sessionId === focusedId)
        ?? ordered.find(row => row.sessionId === cursor);
      if (view === "overview") {
        if (e.key.toLowerCase() === "j" || e.key.toLowerCase() === "k") {
          e.preventDefault();
          const index = ordered.findIndex(row => row.sessionId === (focusedId ?? cursor));
          const next = ordered[Math.max(0, Math.min(index < 0 ? 0 : index + (e.key.toLowerCase() === "j" ? 1 : -1), ordered.length - 1))];
          setCursor(next?.sessionId);
          return;
        }
        if (
          e.key === "Enter" &&
          ordered.length > 0 &&
          !(e.target as HTMLElement).closest('button,[role="button"]')
        ) {
          e.preventDefault();
          select((row ?? ordered[0]).sessionId);
        }
      }
      if (!row || c.busy || target.closest("button,a,[role=menu]")) return;
      if (e.key.toLowerCase() === "o") onOpenSession(row.sessionId);
      if (e.key.toLowerCase() === "c") void c.perform(() => c.openPrompt(row));
      if (e.key.toLowerCase() === "a" && row.state === "review")
        primaryAction(row);
    };
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  });
  const dialogs = (
    <LedgerDialogs controller={c} onOpenSession={onOpenSession} stopRow={stopRow} setStopRow={setStopRow} onStop={onStop} />
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
      subtitle={c.loading && !c.error ? `正在加载最近 ${c.hours} 小时的会话…` : subtitles[view]}
      search={search}
      onSearch={view === "overview" || view === "todos" || view === "goals" ? onSearch : undefined}
      searchPlaceholder={view === "overview" ? "搜索会话、原话、项目" : view === "todos" ? "搜索待办、目标、验收清单" : "搜索目标、待办、成功标准"}
      projects={projects}
      project={project}
      onProject={view === "overview" || view === "todos" || view === "goals" ? onProject : undefined}
      needCount={c.rows.filter((r) => rowPresentation(r).need).length}
      todoCount={allTodos.filter((t) => t.status !== "done").length}
      focusUntil={c.settings.focus.until}
      focusMinutes={c.settings.focus.minutes}
      onFocus={() => void c.perform(c.toggleFocus)}
      onLogout={() => void onLogout()}
      onRefresh={() => void c.perform()}
      syncStatus={c.syncStatus}
      modalOpen={!!(stopRow || c.prompt || c.editing || c.itemEditor || (narrow && c.selectedId && !full))}
      overlay={<>
      {c.selectedId && !c.detail && (
        <LedgerDrawerSkeleton modal={narrow} onClose={onCloseDetail} error={c.detailError} onRetry={() => void c.retryDetail()} busy={c.busy} />
      )}
      {c.detail && (
        <LedgerDrawer
          row={c.detail}
          modal={narrow}
          onClose={onCloseDetail}
          onOpenSession={() => onOpenSession(c.detail!.sessionId)}
          onFullDetail={() => openFull()}
          onSelectSession={select}
          recovery={c.recovery}
          onCopyRecovery={() =>
            void c.perform(() =>
              navigator.clipboard.writeText(c.recovery!.command!),
            )
          }
        />
      )}
      {dialogs}
      </>}
    >
      {c.error && (
        <div className={styles.errorBanner} role="alert">
          {c.error}
          <ActionButton onClick={() => void c.perform()}>
            重试
          </ActionButton>
        </div>
      )}
      {c.loading && !c.error && (view === "overview" ? <LedgerOverviewSkeleton /> : (
        <div className={styles.pageContent} role="status">
          正在读取执行记录…
        </div>
      ))}
      {view === "overview" && pendingCount > 0 && (
        <div className={styles.pendingUpdate}>
          鼠标在面板内，已暂停重排 · 有 {pendingCount} 条更新
        </div>
      )}
      {view === "overview" && c.outsideError && <div className={styles.errorBanner} role="alert">范围外输入／审批请求读取失败：{c.outsideError}</div>}
      {view === "overview" && outside.length > 0 && (
        <details className={styles.outsideNotice} data-blocked>
          <summary>时间范围外还有 {outside.length} 个会话等你回答或审批（含子任务）</summary>
          {(allOutside ? outside : outside.slice(0, 10)).map(row => <div key={row.sessionId}><button onClick={() => select(row.sessionId)}>{row.title} · 等你回答或审批</button></div>)}
          {outside.length > 10 && <button onClick={() => setAllOutside(value => !value)}>{allOutside ? "收起范围外会话" : `展开其余 ${outside.length - 10} 个会话`}</button>}
        </details>
      )}
      {!c.loading && view === "overview" && (
        <LedgerOverview
          rows={filtered}
          groups={groups}
          grouping={grouping}
          onGroupingChange={setGrouping}
          monitor={c.monitor}
          monitorError={c.monitorError}
          hours={c.hours}
          maxHours={c.settings.retentionDays * 24}
          onHoursChange={c.changeHours}
          selectedId={c.selectedId}
          cursor={cursor}
          onSelect={select}
          onAction={primaryAction}
          onMenu={operate}
          busy={c.busy}
          onPause={() => setHeldGroups(current => current ?? groups)}
          onResume={() => setHeldGroups(null)}
        />
      )}
      {!c.loading && view === "todos" && (
        <LedgerTodos controller={scopedController} scoped={!!(query || project)} onSelect={select} />
      )}
      {!c.loading && view === "goals" && (
        <LedgerGoals controller={scopedController} onTodos={() => navigate("todos")} onSelect={select} onOpenSession={onOpenSession} />
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
    </WorkspaceShell>
  );
}
