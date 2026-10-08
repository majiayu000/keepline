import type { LedgerController } from "@/hooks/useLedger";
import type { Goal } from "../types";
import { ActionButton, ProgressSegments, StatusTag } from "./LedgerPrimitives";
import styles from "./GoalProjectMap.module.css";

export function GoalProjectMap({
  goal,
  controller: c,
  onTodos,
  onSelect,
  onOpenSession,
}: {
  goal: Goal;
  controller: LedgerController;
  onTodos: () => void;
  onSelect: (id: string) => void;
  onOpenSession: (id: string) => void;
}) {
  const parts = goal.todos.map((todo) => {
    const rows = c.rows.filter((row) =>
      todo.sessions.some((s) => s.runtime_session_id === row.sessionId),
    );
    const waiting = todo.sessions.find(
      (s) =>
        rows.find((r) => r.sessionId === s.runtime_session_id)?.state ===
          "needs_input" || s.needsInput,
    );
    const running = todo.sessions.find((s) => {
      const row = rows.find((r) => r.sessionId === s.runtime_session_id);
      return row ? row.state === "running" : s.status === "running";
    });
    // An older stopped attempt must not block a newer active or finished run.
    const latest = todo.sessions[0];
    const stopped = !running
      ? rows.find(
          (r) =>
            r.sessionId === latest?.runtime_session_id && r.state === "stopped",
        )
      : undefined;
    const stoppedSession =
      !running &&
      latest &&
      ["lost", "stalled", "interrupted"].includes(latest.status)
        ? latest
        : undefined;
    const evidenced = todo.checklist.filter((i) => i.evidenced).length;
    const pendingReview =
      todo.checklist.length > 0 &&
      evidenced === todo.checklist.length &&
      !todo.readyToComplete;
    const blocked =
      todo.status !== "done" &&
      (todo.status === "blocked" || !!waiting || !!stopped || !!stoppedSession);
    const reason = waiting
      ? rows.find((r) => r.sessionId === waiting.runtime_session_id)
          ?.statusReason ||
        waiting.statusReason ||
        "等待你在 agent 中回答或审批"
      : todo.status === "blocked"
        ? todo.body || "这个待办已标记受阻，尚未记录具体原因"
        : stopped
          ? stopped.statusReason || "关联会话已停下，查看记录后决定是否续做"
          : stoppedSession
            ? stoppedSession.statusReason ||
              "关联会话已停下，查看记录后决定是否续做"
            : "";
    const session =
      waiting ||
      running ||
      (stopped &&
        todo.sessions.find(
          (s) => s.runtime_session_id === stopped.sessionId,
        )) ||
      stoppedSession ||
      todo.sessions[0];
    const status =
      todo.status === "done"
        ? "已验收"
        : blocked
          ? waiting
            ? "等你回答"
            : "受阻 / 已停下"
          : todo.readyToComplete
            ? "可标记完成"
            : pendingReview
              ? "有证据 · 待验收"
              : running || todo.status === "active"
                ? "进行中"
                : evidenced
                  ? "已有执行证据"
                  : todo.sessions.length
                    ? "待续做"
                    : "未开始";
    const color =
      todo.status === "done"
        ? "done"
        : blocked || pendingReview || todo.readyToComplete
          ? "unverified"
          : running || evidenced || todo.status === "active"
            ? "doing"
            : "todo";
    return {
      todo,
      session,
      waiting,
      running,
      blocked,
      reason,
      evidenced,
      pendingReview,
      status,
      color: color as "done" | "unverified" | "doing" | "todo",
    };
  });
  const remaining = parts.filter((p) => p.todo.status !== "done");
  const blockers = remaining.filter((p) => p.blocked);
  const next =
    remaining.find((p) => p.todo.readyToComplete && !p.blocked) ||
    remaining.find((p) => p.pendingReview && !p.blocked) ||
    remaining.find((p) => !p.blocked) ||
    remaining[0];
  const nextText = !parts.length
    ? "先补充这个目标的交付项和验收清单"
    : !next
      ? "所有交付项已验收，可以核对目标的成功标准"
      : next.blocked
        ? `先处理「${next.todo.title}」的卡点`
        : next.todo.readyToComplete
          ? `确认并标记「${next.todo.title}」完成`
          : next.pendingReview
            ? `核对「${next.todo.title}」的执行证据`
            : next.running
              ? `「${next.todo.title}」正在推进，查看当前执行`
              : `继续完成「${next.todo.title}」`;
  const recentLabels: Record<string, string> = {
    test: "测试记录",
    file: "文件改动",
    commit: "提交记录",
    pr: "PR 记录",
    accepted: "验收通过",
    accepted_with_gaps: "带缺口验收",
    completed: "待办完成",
  };

  return (
    <section className={styles.map} aria-label={`项目进度图：${goal.title}`}>
      <header className={styles.header}>
        <div>
          <span className={styles.eyebrow}>
            项目进度图
            {goal.projectRoot &&
              ` · ${goal.projectRoot.split("/").filter(Boolean).at(-1)}`}
          </span>
          <h2>{goal.title}</h2>
          <p>成功标准：{goal.outcome || "尚未设置"}</p>
        </div>
        <ActionButton onClick={() => c.editWorkItem(goal)}>
          编辑目标
        </ActionButton>
      </header>
      <div className={styles.summary}>
        <strong>
          {!parts.length
            ? "尚未定义交付项"
            : remaining.length
              ? `离目标交付还差 ${remaining.length} 项`
              : "全部交付项已验收"}
        </strong>
        <span>
          {goal.progress.done} / {goal.progress.total} 已验收 ·{" "}
          {blockers.length} 项需要处理 · 本周完成 {goal.weeklyMovement} 项
        </span>
        {goal.stale && <StatusTag tag="7 天未推进" bg="#FFF1DB" fg="#92400E" />}
        {!!parts.length && (
          <ProgressSegments
            stretch
            items={parts.map((p) => ({ status: p.color }))}
          />
        )}
      </div>
      <div className={styles.layout}>
        <div>
          <div className={styles.sectionHeading}>
            <h3>交付项</h3>
            <ActionButton onClick={() => c.editWorkItem(undefined, goal.id)}>
              ＋ 加待办
            </ActionButton>
          </div>
          {!parts.length && (
            <p className={styles.empty}>
              添加待办和验收清单后，这里会显示可核对的交付进度。
            </p>
          )}
          <div className={styles.parts}>
            {parts.map((p) => (
              <article
                key={p.todo.id}
                className={`${styles.part} ${p.blocked ? styles.blocked : ""} ${p.todo.status === "done" ? styles.done : ""}`}
              >
                <button
                  className={styles.partTitle}
                  onClick={() => c.editWorkItem(p.todo)}
                >
                  {p.todo.title}
                </button>
                <StatusTag
                  tag={p.status}
                  bg={
                    p.color === "done"
                      ? "#E8F5EE"
                      : p.color === "unverified"
                        ? "#FFF1DB"
                        : "#EEF0FF"
                  }
                  fg={
                    p.color === "done"
                      ? "#166534"
                      : p.color === "unverified"
                        ? "#92400E"
                        : "#4338CA"
                  }
                />
                {p.todo.status !== "done" && (
                  <>
                    {p.todo.checklist.length ? (
                      <>
                        <p>
                          {p.evidenced} / {p.todo.checklist.length} 项有执行证据
                          · {p.todo.checklist.filter((i) => i.satisfied).length}{" "}
                          项已验收
                        </p>
                        <ul className={styles.checklist}>
                          {p.todo.checklist
                            .filter((i) => !i.satisfied)
                            .map((i) => (
                              <li key={i.id}>
                                <span>{i.evidenced ? "待验收" : "待完成"}</span>
                                {i.text}
                              </li>
                            ))}
                        </ul>
                      </>
                    ) : (
                      <p>尚未设置验收清单</p>
                    )}
                    {p.reason && <p className={styles.reason}>{p.reason}</p>}
                  </>
                )}
                <div className={styles.actions}>
                  {p.session && (
                    <ActionButton
                      onClick={() => onSelect(p.session!.runtime_session_id)}
                    >
                      看会话与证据
                    </ActionButton>
                  )}
                  {p.todo.status !== "done" &&
                    (p.todo.readyToComplete && !p.blocked ? (
                      <ActionButton
                        disabled={c.busy}
                        onClick={() =>
                          void c.perform(() => c.completeTodo(p.todo.id))
                        }
                      >
                        标记完成
                      </ActionButton>
                    ) : p.waiting ? (
                      <ActionButton
                        onClick={() =>
                          onOpenSession(p.waiting!.runtime_session_id)
                        }
                      >
                        跳到 agent 回答
                      </ActionButton>
                    ) : p.session ? (
                      <ActionButton
                        disabled={c.busy}
                        onClick={() =>
                          void c.perform(() =>
                            c.openPrompt({
                              sessionId: p.session!.runtime_session_id,
                            }),
                          )
                        }
                      >
                        生成续做提示
                      </ActionButton>
                    ) : (
                      <ActionButton onClick={() => c.prepareDispatch(p.todo)}>
                        交给 agent
                      </ActionButton>
                    ))}
                </div>
              </article>
            ))}
          </div>
        </div>
        <aside className={styles.aside}>
          <section className={styles.panel} aria-label="建议下一步">
            <h3>建议下一步</h3>
            <strong>{nextText}</strong>
            {next && (
              <p>
                {next.reason ||
                  next.todo.checklist
                    .filter((i) => !i.satisfied)
                    .map((i) => i.text)
                    .join("；") ||
                  "对照目标的成功标准核对结果"}
              </p>
            )}
            {next ? (
              <ActionButton
                primary
                onClick={() =>
                  next.session
                    ? onSelect(next.session.runtime_session_id)
                    : c.editWorkItem(next.todo)
                }
              >
                {next.session ? "查看对应会话" : "查看待办"}
              </ActionButton>
            ) : (
              <ActionButton onClick={() => c.editWorkItem(undefined, goal.id)}>
                补充待办
              </ActionButton>
            )}
          </section>
          <section className={styles.panel} aria-label="需要你处理">
            <h3>需要你处理 · {blockers.length}</h3>
            {blockers.length ? (
              blockers.map((p) => (
                <div key={p.todo.id} className={styles.blocker}>
                  <strong>{p.todo.title}</strong>
                  <p>{p.reason}</p>
                  <ActionButton
                    onClick={() =>
                      p.waiting
                        ? onOpenSession(p.waiting.runtime_session_id)
                        : p.session
                          ? onSelect(p.session.runtime_session_id)
                          : c.editWorkItem(p.todo)
                    }
                  >
                    {p.waiting ? "跳到 agent 回答" : "查看卡点"}
                  </ActionButton>
                </div>
              ))
            ) : (
              <p>当前没有明确的审批等待或阻塞记录。</p>
            )}
          </section>
          <section className={styles.panel} aria-label="最近进展">
            <h3>最近进展 · 7 天</h3>
            {goal.recent.length ? (
              <ol className={styles.recent}>
                {goal.recent.map((event) => (
                  <li key={event.id}>
                    <time dateTime={event.at}>
                      {new Date(event.at).toLocaleString("zh-CN", {
                        month: "numeric",
                        day: "numeric",
                        hour: "2-digit",
                        minute: "2-digit",
                        hour12: false,
                      })}
                    </time>
                    <button
                      onClick={() =>
                        event.sessionId
                          ? onSelect(event.sessionId)
                          : c.editWorkItem(
                              goal.todos.find((t) => t.id === event.todoId),
                            )
                      }
                    >
                      <span>{recentLabels[event.kind]}</span>
                      {event.title}
                    </button>
                  </li>
                ))}
              </ol>
            ) : (
              <p>近 7 天暂无关联的执行或验收记录。</p>
            )}
          </section>
        </aside>
      </div>
      <footer className={styles.footer}>
        <span>执行记录用于核对；完成数量以已验收待办为准。</span>
        <ActionButton onClick={onTodos}>查看全部待办</ActionButton>
      </footer>
    </section>
  );
}
