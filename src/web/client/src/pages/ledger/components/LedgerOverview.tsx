import { useEffect, useState } from "react";
import type { LedgerDetail, LedgerMonitorStatus } from "../../../../../../domain/ledger/types";
import { rowPresentation, monitorPresentation, elapsed } from "../presentation";
import {
  ProgressSegments,
  Segmented,
  StatusTag,
  SubtaskTag,
  EmptyState,
} from "./LedgerPrimitives";
import styles from "../Workspace.module.css";
export type OverviewLayout = "grid" | "board" | "list";
function ContextUsage({ context }: { context: ReturnType<typeof rowPresentation>["context"] }) {
  const tooltip = context?.tooltip ?? "上下文用量暂未获取";
  return (
    <span
      className={styles.contextUsage}
      role="progressbar"
      aria-label="上下文占用"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={context ? Math.min(100, context.percent) : undefined}
      aria-valuetext={tooltip}
      tabIndex={0}
    >
      <svg className={styles.contextRing} viewBox="0 0 24 24" aria-hidden="true">
        <circle className={styles.contextRingTrack} cx="12" cy="12" r="9" />
        {context && context.percent > 0 && <circle cx="12" cy="12" r="9" pathLength="100" stroke={context.bar} strokeDasharray={`${Math.min(100, context.percent)} 100`} transform="rotate(-90 12 12)" />}
      </svg>
      <span className={styles.contextPercent}>上下文 {context?.text ?? "—"}</span>
      <span className={styles.contextTooltip} role="tooltip">{tooltip}</span>
    </span>
  );
}
export function savedOverviewView() {
  try {
    return JSON.parse(localStorage.getItem("keepline.overview-view") ?? "{}");
  } catch {
    return {};
  }
}
export function LedgerOverview({
  rows,
  groups,
  grouping,
  onGroupingChange,
  monitor,
  monitorError,
  hours,
  maxHours,
  onHoursChange,
  selectedId,
  onSelect,
  onAction,
  onPause,
  onResume,
  onMenu,
  busy,
  cursor,
}: {
  rows: LedgerDetail[];
  groups: ReturnType<typeof import("../presentation").groupRows>;
  grouping: "recent" | "urgency" | "project" | "goal";
  onGroupingChange: (value: "recent" | "urgency" | "project" | "goal") => void;
  monitor: LedgerMonitorStatus | null;
  monitorError: string;
  hours: number;
  maxHours: number;
  onHoursChange: (hours: number) => void;
  selectedId: string | null;
  onSelect: (id: string) => void;
  onAction: (row: LedgerDetail) => void;
  onPause: () => void;
  onResume: () => void;
  onMenu: (row: LedgerDetail, action: string) => void;
  busy: boolean;
  cursor: string | undefined;
}) {
  const [saved] = useState(savedOverviewView);
  const [draftHours, setDraftHours] = useState(String(hours));
  useEffect(() => setDraftHours(String(hours)), [hours]);
  const [layout, setLayout] = useState<OverviewLayout>(
    ["grid", "board", "list"].includes(saved?.layout) ? saved.layout : "grid",
  );
  const [density, setDensity] = useState<"compact" | "detailed">(
    saved?.density === "detailed" ? "detailed" : "compact",
  );
  const [expandedHistory, setExpandedHistory] = useState<string[]>([]);
  const [collapsedCursor, setCollapsedCursor] = useState<string>();
  useEffect(() => {
    try {
      localStorage.setItem(
        "keepline.overview-view",
        JSON.stringify({ layout, density, grouping }),
      );
    } catch {
      // View preferences are optional when browser storage is unavailable.
    }
  }, [layout, density, grouping]);
  const [menu, setMenu] = useState<string | null>(null);
  useEffect(() => {
    if (!menu) return;
    document.querySelector<HTMLElement>(`[data-row-menu="${CSS.escape(menu)}"] [role="menuitem"]:not(:disabled)`)?.focus();
    const close = (event: PointerEvent) => {
      if ((event.target as HTMLElement).closest<HTMLElement>("[data-row-menu]")?.dataset.rowMenu !== menu) setMenu(null);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      setMenu(null);
      document.querySelector<HTMLElement>(`[data-row-menu="${CSS.escape(menu)}"] > button`)?.focus();
    };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", escape, true);
    return () => { document.removeEventListener("pointerdown", close); document.removeEventListener("keydown", escape, true); };
  }, [menu]);
  const health = monitor ? monitorPresentation(monitor) : null;
  return (
    <>
      <details className={styles.monitorStatus} data-warning={!!monitorError || health?.warning || undefined}>
        <summary>{monitorError ? `无法确认监测状态：${monitorError}` : health?.warning ? health.text : "监测状态"}</summary>
        <div>{health?.text ?? "正在读取监测状态…"}</div>
        {monitor?.runtimes.map(runtime => (
          <div key={runtime.runtimeId}>
            <strong>{runtime.runtimeId === "codex" ? "Codex" : "Claude Code"}</strong>
            {" · "}{runtime.lastScanAt ? `日志扫描 ${elapsed(runtime.lastScanAt)}前` : "尚无日志扫描结果"}
            {runtime.errorCount > 0 && ` · ${runtime.errorCount} 条读取失败`}
            {" · "}{!runtime.receiverRunning ? "实时接收不可用"
              : runtime.lastHookAt ? `收到实时事件 ${elapsed(runtime.lastHookAt)}前`
              : runtime.hookInstalled ? "实时通知已安装，尚未收到事件" : "仅日志监测，未安装实时通知"}
          </div>
        ))}
      </details>
      <div className={styles.overviewToolbar}>
        <Segmented
          label="布局"
          value={layout}
          onChange={setLayout}
          options={[
            ["grid", "网格"],
            ["board", "看板"],
            ["list", "列表"],
          ]}
        />
        <Segmented
          label="密度"
          value={density}
          onChange={setDensity}
          options={[
            ["compact", "紧凑"],
            ["detailed", "详细"],
          ]}
        />
        <Segmented
          label="分组"
          value={grouping}
          onChange={onGroupingChange}
          options={[
            ["recent", "最新活动"],
            ["urgency", "按紧急度"],
            ["project", "按项目"],
            ["goal", "按目标"],
          ]}
        />
        <form
          className={styles.timeRange}
          onSubmit={(event) => {
            event.preventDefault();
            onHoursChange(Number(draftHours));
          }}
        >
          <label htmlFor="overview-hours">最近</label>
          <input
            id="overview-hours"
            aria-label="最近活动小时数"
            type="number"
            min={1}
            max={maxHours}
            step={1}
            required
            value={draftHours}
            onChange={(event) => setDraftHours(event.target.value)}
          />
          <span>小时内有活动</span>
          <button type="submit">应用</button>
        </form>
        <span className={styles.keyboardHint}>
          J/K 上下 · Enter 详情 · O 查看会话记录 · A 验收 · C 复制追问
        </span>
      </div>
      {!rows.length && (
        <div className={styles.pageContent}>
          <EmptyState>
            这个时间段暂无会话。可在设置检查排除项与记录保留时间。
          </EmptyState>
        </div>
      )}
      <div
        className={`${styles.overviewGroups} ${layout === "board" ? styles.board : ""}`}
        onPointerEnter={onPause}
        onPointerLeave={onResume}
      >
        {groups.map((g) => {
          const history = g.rows.filter((r) => rowPresentation(r).historical);
          const expanded =
            expandedHistory.includes(g.key) ||
            (cursor !== collapsedCursor &&
              history.slice(4).some((r) => r.sessionId === cursor));
          const visibleHistory = new Set((expanded ? history : history.slice(0, 4)).map(r => r.sessionId));
          const visible = g.rows.filter(r => !rowPresentation(r).historical || visibleHistory.has(r.sessionId));
          return (
            <section key={g.key} className={styles.overviewGroup}>
              <div className={styles.groupHeading}>
                <h2>{g.label}</h2>
                <span>{g.rows.length}</span>
                <span>{g.hint}</span>
              </div>
              <div
                className={`${styles.cards} ${layout === "list" ? styles.list : layout === "board" ? styles.boardCards : density === "detailed" ? styles.detailedCards : ""}`}
              >
                {visible.map((row) => {
                  const p = rowPresentation(row);
                  const active = selectedId === row.sessionId;
                  const actions = [
                    "查看会话记录",
                    "看进度账",
                    ...(row.items.length ? ["复制追问"] : []),
                    ...(row.state === "running" || row.state === "needs_input"
                      ? ["停止进程"]
                      : []),
                    ...(row.state === "review" && row.progress.total > 0
                      ? ["验收通过", "带着缺口验收"]
                      : []),
                  ];
                  const menuControl = (
                    <span className={styles.rowMenu} data-row-menu={row.sessionId}>
                        <button
                          className={styles.cardMenuTrigger}
                          onClick={(e) => {
                            e.stopPropagation();
                            setMenu(
                              menu === row.sessionId ? null : row.sessionId,
                            );
                          }}
                          aria-label={`更多操作：${row.title}`}
                          aria-haspopup="menu"
                          aria-expanded={menu === row.sessionId}
                        >
                          <i
                            aria-hidden="true"
                            className="ph-duotone ph-dots-three"
                          />
                        </button>
                      {menu === row.sessionId && (
                        <span
                          role="menu"
                          aria-label={`会话操作：${row.title}`}
                          className={styles.cardMenu}
                          onClick={(e) => e.stopPropagation()}
                          onKeyDown={(event) => {
                            if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
                            event.preventDefault();
                            event.stopPropagation();
                            const items = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
                            const index = items.indexOf(document.activeElement as HTMLButtonElement);
                            const next = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1
                              : (index + (event.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
                            items[next]?.focus();
                          }}
                        >
                          {actions.map((action) => (
                            <button
                              role="menuitem"
                              disabled={busy}
                              key={action}
                              onClick={(event) => {
                                event.currentTarget.closest("[data-row-menu]")?.querySelector<HTMLButtonElement>("button")?.focus();
                                setMenu(null);
                                onMenu(row, action);
                              }}
                            >
                              {action}
                            </button>
                          ))}
                        </span>
                      )}
                    </span>
                  );
                  const dot = (
                    <span
                      className={`${styles.activityDot} ${row.state === "running" && !p.need ? styles.liveDot : ""}`}
                      style={{
                        background: p.highlight
                          ? "#F59E0B"
                          : row.state === "running"
                            ? "#4F5BD5"
                            : "#C9CDD6",
                        boxShadow: p.highlight
                          ? "0 0 0 3px rgba(245,158,11,.18)"
                          : row.state === "running"
                            ? "0 0 0 3px rgba(79,91,213,.16)"
                            : "none",
                      }}
                    />
                  );
                  if (layout === "list")
                    return (
                      <div
                        role="button"
                        tabIndex={0}
                        onKeyDown={(event) => {
                          if (event.target === event.currentTarget && (event.key === "Enter" || event.key === " ")) {
                            event.preventDefault();
                            event.stopPropagation();
                            onSelect(row.sessionId);
                          }
                        }}
                        key={row.sessionId}
                        aria-label={row.title}
                        data-session-id={row.sessionId}
                        data-cursor={cursor === row.sessionId || undefined}
                        className={styles.listRow}
                        style={{ background: active ? "#F2F3F6" : "#fff" }}
                        onClick={() => onSelect(row.sessionId)}
                      >
                        <StatusTag tag={p.tag} bg={p.tagBg} fg={p.tagFg} title={p.observationTitle} />
                        <span className={styles.mono} title={p.activityTitle}>
                          {p.activeAgo}
                        </span>
                        <span className={styles.listTitle}>
                          <strong>{row.title}</strong>
                          {(row.isSubAgent || row.parentSessionId) && <SubtaskTag />}
                          {!!row.subagents?.length && <SubtaskTag count={row.subagents.length} />}
                          <span>你说：“{p.ask}”</span>
                        </span>
                        <span className={styles.listActivity}>
                          {dot}
                          <span>{p.now}</span>
                          <span className={styles.subtle}>· {p.lastAgo}</span>
                        </span>
                        <span className={styles.listProgress}>
                          {p.items.length > 0 ? (
                            <>
                              <ProgressSegments items={p.items} />
                              <span className={styles.mono}>
                                <span>{`${row.progress.done} / ${row.progress.total}`}</span>
                              </span>
                            </>
                          ) : (
                            <span className={styles.fallback}>兜底项</span>
                          )}
                          <ContextUsage context={p.context} />
                          {menuControl}
                        </span>
                      </div>
                    );
                  return (
                    <div
                      key={row.sessionId}
                      role="button"
                      tabIndex={0}
                      aria-label={row.title}
                      data-session-id={row.sessionId}
                      data-cursor={cursor === row.sessionId || undefined}
                      className={`${styles.sessionCard} ${p.highlight ? styles.attentionCard : ""} ${p.historical || p.paused ? styles.quietCard : ""} ${active ? styles.selectedCard : ""}`}
                      onClick={() => onSelect(row.sessionId)}
                      onKeyDown={(e) => {
                        if (
                          e.target === e.currentTarget &&
                          (e.key === "Enter" || e.key === " ")
                        ) {
                          e.preventDefault();
                          e.stopPropagation();
                          onSelect(row.sessionId);
                        }
                      }}
                    >
                      <span className={styles.cardHeader}>
                        <StatusTag tag={p.tag} bg={p.tagBg} fg={p.tagFg} title={p.observationTitle} />
                        {(row.isSubAgent || row.parentSessionId) && <SubtaskTag />}
                        <span className={styles.cardAgent} title={p.agent}>
                          {p.agent}
                        </span>
                        <span
                          className={`${styles.mono} ${styles.cardElapsed}`}
                          title={p.activityTitle}
                        >
                          {p.activeAgo}
                        </span>
                        {menuControl}
                      </span>
                      <span className={styles.cardTitle} title={row.title}>
                        {row.title}
                      </span>
                      <span className={styles.cardAsk} title={p.ask}>
                        你说：“<span>{p.ask}</span>”
                      </span>
                      {p.items.length > 0 && (
                        <span className={styles.cardProgress}>
                          <ProgressSegments items={p.items} stretch />
                          <span className={styles.mono}>
                            <span>{`${row.progress.done} / ${row.progress.total}`}</span>
                          </span>
                        </span>
                      )}
                      <span className={styles.cardNow}>
                        {!p.ended ? dot : null}
                        <span
                          className={
                            p.ended
                              ? `${styles.evidence} ${styles.cardEvidence}`
                              : undefined
                          }
                          title={p.ended ? p.evidence : p.now}
                        >
                          {p.ended ? p.evidence : p.now}
                        </span>
                        <span>{p.lastAgo}</span>
                      </span>
                      <ContextUsage context={p.context} />
                      {p.action && (
                        <button
                          disabled={busy}
                          className={`${styles.cardAction} ${row.state === "needs_input" ? styles.primary : ""}`}
                          onClick={(e) => {
                            e.stopPropagation();
                            onAction(row);
                          }}
                        >
                          {p.action}
                        </button>
                      )}
                      {!!row.subagents?.length && <SubtaskTag count={row.subagents.length} />}
                    </div>
                  );
                })}
              </div>
              {history.length > 4 && (
                <button
                  className={styles.historyToggle}
                  aria-expanded={expanded}
                  onClick={() => {
                    if (expanded) setCollapsedCursor(cursor);
                    setExpandedHistory((keys) =>
                      expanded
                        ? keys.filter((key) => key !== g.key)
                        : [...keys, g.key],
                    );
                  }}
                >
                  {expanded
                    ? "收起历史会话 ↑"
                    : `查看其余 ${history.length - 4} 条历史会话 ↓`}
                </button>
              )}
            </section>
          );
        })}
      </div>
    </>
  );
}
