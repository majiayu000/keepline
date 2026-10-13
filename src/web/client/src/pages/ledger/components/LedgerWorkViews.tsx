import { lazy, Suspense, useEffect, useState } from "react";
import type { LedgerController } from "@/hooks/useLedger";
import type { LedgerDetail } from "../../../../../../domain/ledger/types";
import type { Todo } from "../types";
import {
  itemColors,
  rowPresentation,
  evidenceText,
  time,
  reviewAcceptanceAt,
} from "../presentation";
import {
  ActionButton,
  EmptyState,
  ProgressSegments,
  Segmented,
  StatusTag,
} from "./LedgerPrimitives";
import styles from "../Workspace.module.css";
const GoalProjectMap = lazy(() => import("./GoalProjectMap").then(module => ({ default: module.GoalProjectMap })));

export function LedgerTodos({
  controller: c,
  onSelect,
  scoped = false,
}: {
  controller: LedgerController;
  onSelect: (id: string) => void;
  scoped?: boolean;
}) {
  const [filter, setFilter] = useState<"all" | "open" | "unassigned">("all");
  const [ignoredSuggestions, setIgnoredSuggestions] = useState<string[]>(() => {
    try {
      const saved: unknown = JSON.parse(localStorage.getItem("keepline.ignored-followups") ?? "[]");
      return Array.isArray(saved) ? saved.filter((key): key is string => typeof key === "string") : [];
    }
    catch { return []; }
  });
  useEffect(() => {
    try { localStorage.setItem("keepline.ignored-followups",JSON.stringify(ignoredSuggestions)); }
    catch { /* Ignoring still works for this visit when storage is unavailable. */ }
  },[ignoredSuggestions]);
  const sources = c.rows.flatMap(row => [row,...(row.subagents ?? []).map(child => ({ ...child,workItemId: child.workItemId ?? row.workItemId }))]);
  const suggestions = sources.flatMap(row => {
    const todo = c.goals.flatMap(g => g.todos).find(t => t.id === row.workItemId) ?? c.todos.find(t => t.id === row.workItemId);
    const parentId = c.goals.some(g => g.id === todo?.parentId) ? todo!.parentId! : "unassigned";
    return (row.followUpSuggestions ?? []).map(text => ({ key: JSON.stringify([row.sessionId,text]),text,row,parentId }));
  }).filter(s => !ignoredSuggestions.includes(s.key) && !c.todos.some(t => t.title === s.text && (t.parentId ?? "unassigned") === s.parentId));
  const todoGroups = [
    ...c.goals.map((g) => ({
      id: g.id,
      title: g.title,
      todos: g.todos,
      weekly: g.weeklyMovement,
      stale: g.stale,
      inactiveDays: g.inactiveDays,
    })),
    {
      id: "unassigned",
      title: "未关联目标",
      weekly: 0,
      stale: false,
      inactiveDays: 0,
      todos: c.todos
        .filter((t) => !c.goals.some(goal => goal.id === t.parentId)),
    },
  ];
  const dispatch = (todo: Todo) => c.prepareDispatch(todo);
  const totalSteps = c.rows.reduce((count, row) => count + row.trail.length, 0);
  const offPlanRows = c.rows.filter(row => row.offPlan.length > 0);
  const offPlanSteps = offPlanRows.reduce((count, row) => count + new Set(row.offPlan.flatMap(run => run.callIds)).size, 0);
  return (
    <div
      className={`${styles.pageContent} ${styles.todosContent}`}
      style={{ gap: 26 }}
    >
      <div className={styles.spaceBetween}>
        <Segmented
          label="待办筛选"
          value={filter}
          onChange={setFilter}
          options={[
            ["all", "全部"],
            ["open", "未完成"],
            ["unassigned", "还没派出"],
          ]}
        />
        <ActionButton primary onClick={() => c.editWorkItem()}>
          ＋ 加待办
        </ActionButton>
      </div>
      {todoGroups.map((g) => {
        const todos = g.todos.filter(
          (t) =>
            (filter !== "open" || t.status !== "done") &&
            (filter !== "unassigned" || !t.sessions.length),
        );
        const inferred = suggestions.filter(s => s.parentId === g.id);
        if (!todos.length && !inferred.length) return null;
        return (
          <section key={g.id}>
            <div className={styles.sectionHeading}>
              <h2>{g.title}</h2>
              <ProgressSegments
                items={g.todos.map((t) => ({
                  status:
                    t.status === "done"
                      ? "done"
                      : t.sessions.length
                        ? "doing"
                        : "todo",
                }))}
              />
              <span className={styles.subtle}>
                {`${g.todos.filter((t) => t.status === "done").length} / ${g.todos.length} 已验收 · 本周 +${g.weekly}`}
              </span>
              {g.stale && (
                <span className={styles.staleGoal}>▲ {g.inactiveDays} 天没推进</span>
              )}
            </div>
            <div className={styles.tableCard}>
              {todos.map((todo) => {
                const session = todo.sessions[0];
                const row = c.rows.find(
                  (r) => r.sessionId === session?.runtime_session_id,
                );
                const p = row && rowPresentation(row);
                const state =
                  todo.status === "done"
                    ? "done"
                    : row?.state === "review" &&
                        row.progress.done < row.progress.total
                      ? "unverified"
                      : session
                        ? "doing"
                        : "todo";
                return (
                  <div key={todo.id} className={styles.todoRow}>
                    <span
                      className={styles.square}
                      style={{
                        background: itemColors[state],
                        borderRadius:
                          state === "doing" ? "50%" : state === "todo" ? 2 : 0,
                      }}
                    />
                    <span>
                      <button
                        className={styles.todoTitle}
                        onClick={() => c.editWorkItem(todo)}
                      >
                        {todo.title}
                      </button>
                      <span className={styles.subtle}>
                        {state === "done"
                          ? "已验收"
                          : state === "unverified"
                            ? "待核对 · 有缺口"
                            : state === "doing"
                              ? "进行中"
                              : "未开始"}
                      </span>
                    </span>
                    <span className={styles.todoChecklist}>
                      <ProgressSegments
                        items={todo.checklist.map((i) => ({
                          status: i.satisfied ? "done" : "todo",
                        }))}
                      />
                      <span className={styles.mono}>
                        {`清单 ${todo.checklist.filter((i) => i.satisfied).length} / ${todo.checklist.length}`}
                      </span>
                    </span>
                    {session ? (
                      <button
                        className={styles.todoSession}
                        onClick={() => onSelect(session.runtime_session_id)}
                      >
                        {p ? (
                          <StatusTag tag={p.tag} bg={p.tagBg} fg={p.tagFg} />
                        ) : (
                          <span>{session.status}</span>
                        )}
                        <span>
                          {p ? `${p.agent} · ${p.elapsed}` : session.title}
                        </span>
                      </button>
                    ) : (
                      <span className={styles.subtle}>还没派出</span>
                    )}
                    {!session ? (
                      <ActionButton onClick={() => dispatch(todo)}>
                        交给 Codex ▾
                      </ActionButton>
                    ) : todo.readyToComplete ? (
                      <ActionButton
                        primary
                        disabled={c.busy}
                        onClick={() =>
                          void c.perform(() => c.completeTodo(todo.id))
                        }
                      >
                        标记完成
                      </ActionButton>
                    ) : (
                      <span />
                    )}
                  </div>
                );
              })}
              {inferred.map(suggestion => (
                <div key={suggestion.key} className={styles.todoSuggestion}>
                  <span className={styles.suggestionLabel}>推断</span>
                  <span title={suggestion.text}>{suggestion.row.agentRole ?? (suggestion.row.runtimeId === "codex" ? "Codex" : "Claude Code")} 提出：{suggestion.text}</span>
                  <ActionButton onClick={() => c.setEditing({
                    title: suggestion.text, level: "task", parentId: g.id === "unassigned" ? "" : g.id,
                    outcome: "", checklist: "", projectRoot: suggestion.row.projectRoot,
                  })}>加为待办</ActionButton>
                  <button className={styles.ignoreSuggestion} aria-label={`忽略建议：${suggestion.text}`} onClick={() => setIgnoredSuggestions(keys => [...keys,suggestion.key])}>忽略</button>
                </div>
              ))}
            </div>
          </section>
        );
      })}
      {!todoGroups.some(g => g.todos.some(todo => (filter !== "open" || todo.status !== "done") && (filter !== "unassigned" || !todo.sessions.length))) && !suggestions.length && (
        <EmptyState>{scoped || filter !== "all" ? "没有符合当前筛选的待办。可清空搜索或切换项目。" : "还没有待办，添加任务和验收清单后可派给 agent。"}</EmptyState>
      )}
      <div className={styles.subtle}>
        当前最近 {c.hours} 小时计划外工作 {totalSteps ? Math.round(offPlanSteps / totalSteps * 100) : 0}%
        （{offPlanSteps} 步 / {totalSteps} 步），分布在 {offPlanRows.length} 个会话
        {offPlanRows.length > 0 && <details>
          <summary>归到目标</summary>
          {offPlanRows.map(row => <div key={row.sessionId}><button className={styles.reviewTitle} onClick={() => onSelect(row.sessionId)}>{row.title}</button></div>)}
        </details>}
      </div>
    </div>
  );
}
export function LedgerGoals({
  controller: c,
  onTodos,
  onSelect,
  onOpenSession,
}: {
  controller: LedgerController;
  onTodos: () => void;
  onSelect: (id: string) => void;
  onOpenSession: (id: string) => void;
}) {
  return (
    <div className={styles.pageContent} style={{ gap: 12 }}>
      {c.goals.map(goal => <section key={goal.id}>
        <div className={styles.goalRow}>
          <span style={{ minWidth: 0 }}><button className={styles.goalTitle} onClick={onTodos}>{goal.title}</button><span className={styles.goalOutcome}>成功标准：{goal.outcome || "尚未设置"}</span></span>
          <span className={styles.cardProgress}><ProgressSegments stretch items={goal.todos.map(todo => ({ status: todo.status === "done" ? "done" : todo.sessions.length ? "doing" : "todo" }))} /><span className={styles.mono}>{goal.progress.done} / {goal.progress.total}</span></span>
          <span className={styles.goalMeta}>
            <span>{goal.stale ? <span className={styles.staleGoal}>▲ {goal.inactiveDays} 天没推进</span> : `本周 +${goal.weeklyMovement} · ${goal.progress.active} 项在推进`}</span>
            <button className={`${styles.cardMenuTrigger} ${styles.goalEdit}`} aria-label={`编辑目标：${goal.title}`} onClick={() => c.editWorkItem(goal)}>⋯</button>
            <div style={{ marginTop: 8 }}><button className={styles.actionButton} aria-expanded={c.projectMapGoalId === goal.id} aria-controls={`project-map-${goal.id}`} onClick={() => c.setProjectMapGoalId(c.projectMapGoalId === goal.id ? null : goal.id)}>{c.projectMapGoalId === goal.id ? "收起项目进度图" : "查看项目进度图"}</button></div>
          </span>
        </div>
        {c.projectMapGoalId === goal.id && <div id={`project-map-${goal.id}`} style={{ marginTop: 12 }}><Suspense fallback={<div role="status">正在打开项目进度图…</div>}><GoalProjectMap goal={goal} controller={c} onTodos={onTodos} onSelect={onSelect} onOpenSession={onOpenSession} /></Suspense></div>}
      </section>)}
      {!c.goals.length && <EmptyState>创建目标，写下成功标准，再把待办关联到目标。</EmptyState>}
      <div className={styles.subtle}>一格 = 一个待办。深蓝已验收，浅蓝进行中，橙色待核对有缺口，灰未开始。点目标名进入待办页。</div>
      <div><ActionButton onClick={() => { c.editWorkItem(); c.setEditing(item => item && { ...item, level: "goal" }); }}>＋ 加目标</ActionButton></div>
    </div>
  );
}
export function LedgerReview({
  controller: c,
  onSelect,
  onOpenSession,
}: {
  controller: LedgerController;
  onSelect: (id: string) => void;
  onOpenSession: (id: string) => void;
}) {
  const review = c.review;
  const reviewDate = new Date(`${c.date}T12:00:00`);
  const today = c.date === new Date().toLocaleDateString("en-CA");
  const dateLabel = `${reviewDate.getMonth() + 1} 月 ${reviewDate.getDate()} 日 周${"日一二三四五六"[reviewDate.getDay()]}${today ? " · 今天" : ""}`;
  const move = (days: number) => {
    const d = new Date(`${c.date}T12:00:00`);
    d.setDate(d.getDate() + days);
    c.setDate(d.toLocaleDateString("en-CA"));
  };
  const exportReview = () => {
    if (!review) return;
    const markdown = `# ${c.date} 进度回顾\n\n## 还挂着\n${review.open.map((r) => `- ${r.title}：${r.progress.done}/${r.progress.total}`).join("\n")}\n\n## 已验收\n${review.accepted.map((r) => `- ${r.title}：${evidenceText(r)}`).join("\n")}\n\n## 偏离和纠正\n${review.offPlan.map((r) => `- ${r.title}`).join("\n")}\n\n未关联目标的运行时间：${Math.round(review.unattributedRuntimeShare * 100)}%\n`;
    const url = URL.createObjectURL(
      new Blob([markdown], { type: "text/markdown;charset=utf-8" }),
    );
    const a = document.createElement("a");
    a.href = url;
    a.download = `keepline-review-${c.date}.md`;
    a.click();
    URL.revokeObjectURL(url);
  };
  const rows = (label: string, data: LedgerDetail[], accepted = false) => (
    <section>
      <div className={styles.sectionHeading}>
        <h2>{label}</h2>
        <span className={styles.subtle}>{data.length}</span>
      </div>
      <div className={styles.tableCard}>
        {data.map((row) => (
          <div key={row.sessionId} className={styles.reviewRow}>
            <span>
              <button
                className={styles.reviewTitle}
                onClick={() => onSelect(row.sessionId)}
              >
                {row.title}
              </button>
              <span className={styles.subtle} style={{ marginLeft: 8 }}>
                {accepted ? (() => {
                  const at = review && reviewAcceptanceAt(row, review.start, review.end);
                  return at ? time(at) : "—";
                })() : rowPresentation(row).agent}
              </span>
            </span>
            <span className={`${styles.reviewDetail} ${styles.subtle}`}>
              <span className={accepted ? styles.evidence : undefined}>
                {accepted
                  ? evidenceText(row)
                  : `还差：${
                      row.items
                        .filter(
                          (i) =>
                            !i.dropped &&
                            (i.status !== "done" || !i.evidenceIds.length),
                        )
                        .map((i) => i.title)
                        .join("、") || rowPresentation(row).now
                    }`}
              </span>
            </span>
            <span className={styles.actions}>
              {!accepted && row.state !== "needs_input" && row.progress.total > 0 && (
                <>
                  <ActionButton
                    onClick={() => void c.perform(() => c.openPrompt(row))}
                  >
                    复制续做提示
                  </ActionButton>
                  <ActionButton
                    onClick={() =>
                      void c.perform(() => c.carryOver(row.sessionId))
                    }
                  >
                    带到明天
                  </ActionButton>
                </>
              )}
              {!accepted && row.state === "needs_input" && (
                <ActionButton onClick={() => onOpenSession(row.sessionId)}>
                  查看会话记录
                </ActionButton>
              )}
            </span>
          </div>
        ))}
      </div>
    </section>
  );
  return (
    <div className={`${styles.pageContent} ${styles.reviewContent}`}>
      <div className={styles.reviewToolbar}>
        <button
          className={styles.reviewArrow}
          aria-label={c.weekly ? "上一周" : "前一天"}
          onClick={() => move(c.weekly ? -7 : -1)}
        >
          ‹
        </button>
        <details
          className={styles.settingEditor}
          onKeyDown={(e) => {
            if (e.key === "Escape") e.currentTarget.open = false;
          }}
        >
          <summary className={styles.reviewDate}>
            {dateLabel}
            {c.weekly ? " · 按周" : ""}
            <span className={styles.dateChevron} aria-hidden="true">
              {" "}
              ▾
            </span>
          </summary>
          <div className={styles.settingPopover}>
            <input
              aria-label="回顾日期"
              type="date"
              value={c.date}
              onChange={(e) => c.setDate(e.target.value)}
              className={styles.settingValue}
              style={{ width: 150 }}
            />
            <Segmented
              label="回顾范围"
              value={c.weekly ? "week" : "date"}
              options={[
                ["date", "按天"],
                ["week", "按周"],
              ]}
              onChange={(value) => c.setWeekly(value === "week")}
            />
            {review && (
              <span className={styles.subtle}>
                未关联目标的运行时间：
                {Math.round(review.unattributedRuntimeShare * 100)}%
              </span>
            )}
          </div>
        </details>
        <button
          className={styles.reviewArrow}
          aria-label={c.weekly ? "下一周" : "后一天"}
          onClick={() => move(c.weekly ? 7 : 1)}
        >
          ›
        </button>
        <span className={styles.subtle}>
          {review
            ? `${review.accepted.length} 个已验收 · ${review.open.length} 个还挂着 · ${review.offPlan.length} 次偏离`
            : c.error
              ? "回顾加载失败，请重试"
              : "正在读取回顾…"}
        </span>
        <ActionButton onClick={exportReview}>导出 Markdown</ActionButton>
      </div>
      {review && (
        <>
          {rows("还挂着", review.open)}
          {rows("已验收", review.accepted, true)}
          <section>
            <div className={styles.sectionHeading}>
              <h2>今天的偏离和纠正</h2>
              <span className={styles.subtle}>
                {review.offPlan.length + review.corrections.length}
              </span>
            </div>
            <div className={styles.tableCard}>
              {review.offPlan.map((run) => (
                <div key={run.id} className={styles.reviewRow}>
                  <strong>{run.title}</strong>
                  <span className={styles.subtle}>可能偏离已确认要求</span>
                  <ActionButton onClick={() => onSelect(run.sessionId)}>
                    看进度账
                  </ActionButton>
                </div>
              ))}
              {review.corrections.map((correction, i) => (
                <div key={i} className={styles.reviewRow}>
                  <strong>{correction.title}</strong>
                  <span className={styles.subtle}>
                    已纠正步骤归属 ·{" "}
                    {correction.created_at ? time(correction.created_at) : ""}
                  </span>
                </div>
              ))}
            </div>
          </section>
        </>
      )}
    </div>
  );
}
