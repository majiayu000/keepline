import { useEffect, useState } from "react";
import type { LedgerDetail } from "../../../../../../domain/ledger/types";
import type { Goal, Todo } from "../types";
import { groupRows, rowPresentation } from "../presentation";
import {
  ProgressSegments,
  Segmented,
  StatusTag,
  EmptyState,
} from "./LedgerPrimitives";
import styles from "../Workspace.module.css";
export type OverviewLayout = "grid" | "board" | "list";
function savedView() {
  try {
    return JSON.parse(localStorage.getItem("keepline.overview-view") ?? "{}");
  } catch {
    return {};
  }
}
export function LedgerOverview({
  rows,
  goals,
  todos,
  hours,
  maxHours,
  onHoursChange,
  selectedId,
  onSelect,
  onAction,
  onPause,
  onResume,
  onMenu,
  cursor,
}: {
  rows: LedgerDetail[];
  goals: Goal[];
  todos: Todo[];
  hours: number;
  maxHours: number;
  onHoursChange: (hours: number) => void;
  selectedId: string | null;
  onSelect: (id: string) => void;
  onAction: (row: LedgerDetail) => void;
  onPause: () => void;
  onResume: () => void;
  onMenu: (row: LedgerDetail, action: string) => void;
  cursor: string | undefined;
}) {
  const [saved] = useState(savedView);
  const [draftHours, setDraftHours] = useState(String(hours));
  useEffect(() => setDraftHours(String(hours)), [hours]);
  const [layout, setLayout] = useState<OverviewLayout>(
    ["grid", "board", "list"].includes(saved?.layout) ? saved.layout : "grid",
  );
  const [density, setDensity] = useState<"compact" | "detailed">(
    saved?.density === "detailed" ? "detailed" : "compact",
  );
  const [grouping, setGrouping] = useState<"urgency" | "project" | "goal">(
    saved?.grouping === "project" || saved?.grouping === "goal" ? saved.grouping : "urgency",
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
  const groups = groupRows(rows, grouping, Date.now(), { goals, todos });
  return (
    <>
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
          onChange={setGrouping}
          options={[
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
          J/K 上下 · Enter 详情 · O 跳到 agent · A 验收 · C 复制追问
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
          const current = g.rows.filter((r) => !rowPresentation(r).ended);
          const history = g.rows
            .filter((r) => rowPresentation(r).ended)
            .sort(
              (a, b) => Date.parse(b.lastActiveAt) - Date.parse(a.lastActiveAt),
            );
          const expanded =
            expandedHistory.includes(g.key) ||
            (cursor !== collapsedCursor &&
              history.slice(4).some((r) => r.sessionId === cursor));
          const visible = [
            ...current,
            ...(expanded ? history : history.slice(0, 4)),
          ];
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
                    "跳到 agent",
                    "看进度账",
                    ...(row.items.length ? ["复制追问"] : []),
                    ...(row.state === "running" || row.state === "needs_input"
                      ? ["停止"]
                      : []),
                    ...(row.state === "review" && row.progress.total > 0
                      ? ["验收通过", "带着缺口验收"]
                      : []),
                  ];
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
                      <button
                        key={row.sessionId}
                        aria-label={row.title}
                        className={styles.listRow}
                        style={{ background: active ? "#F2F3F6" : "#fff" }}
                        onClick={() => onSelect(row.sessionId)}
                      >
                        <StatusTag tag={p.tag} bg={p.tagBg} fg={p.tagFg} />
                        <span className={styles.mono} title={p.activityTitle}>
                          {p.activeAgo}
                        </span>
                        <span className={styles.listTitle}>
                          <strong>{row.title}</strong>
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
                        </span>
                      </button>
                    );
                  return (
                    <div
                      key={row.sessionId}
                      role="button"
                      tabIndex={0}
                      aria-label={row.title}
                      data-cursor={cursor === row.sessionId || undefined}
                      className={`${styles.sessionCard} ${p.highlight ? styles.attentionCard : ""} ${p.ended || p.paused ? styles.quietCard : ""} ${active ? styles.selectedCard : ""}`}
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
                        <StatusTag tag={p.tag} bg={p.tagBg} fg={p.tagFg} />
                        <span className={styles.cardAgent} title={p.agent}>
                          {p.agent}
                        </span>
                        <span
                          className={`${styles.mono} ${styles.cardElapsed}`}
                          title={p.activityTitle}
                        >
                          {p.activeAgo}
                        </span>
                        <button
                          className={styles.cardMenuTrigger}
                          onClick={(e) => {
                            e.stopPropagation();
                            setMenu(
                              menu === row.sessionId ? null : row.sessionId,
                            );
                          }}
                          aria-label={`更多操作：${row.title}`}
                          aria-expanded={menu === row.sessionId}
                        >
                          <i
                            aria-hidden="true"
                            className="ph-duotone ph-dots-three"
                          />
                        </button>
                      </span>
                      {menu === row.sessionId && (
                        <div
                          className={styles.cardMenu}
                          onClick={(e) => e.stopPropagation()}
                        >
                          {actions.map((action) => (
                            <button
                              key={action}
                              onClick={() => {
                                setMenu(null);
                                onMenu(row, action);
                              }}
                            >
                              {action}
                            </button>
                          ))}
                        </div>
                      )}
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
                      {p.action && (
                        <button
                          className={`${styles.cardAction} ${row.state === "needs_input" ? styles.primary : ""}`}
                          onClick={(e) => {
                            e.stopPropagation();
                            onAction(row);
                          }}
                        >
                          {p.action}
                        </button>
                      )}
                      {!!row.subagents?.length && density === "detailed" && (
                        <span className={styles.subtle}>
                          {row.subagents.length} 个子任务
                        </span>
                      )}
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
                    : `查看其余 ${history.length - 4} 个已结束会话 ↓`}
                </button>
              )}
            </section>
          );
        })}
      </div>
    </>
  );
}
