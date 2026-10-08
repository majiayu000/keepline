import { useState } from "react";
import type { LedgerController } from "@/hooks/useLedger";
import type { LedgerDetail } from "../../../../../../domain/ledger/types";
import { confirmedRequirement } from "../../../../../../domain/ledger/types";
import {
  rowPresentation,
  groupRows,
  time,
  itemColors,
  requirementStatus,
  sourceText,
  projectName,
  evidenceSummary,
  keyStep,
} from "../presentation";
import { ActionButton, ProgressSegments, StatusTag } from "./LedgerPrimitives";
import styles from "../Workspace.module.css";

export function LedgerAgentPanel({
  controller: c,
  onBack,
  onOpenSession,
}: {
  controller: LedgerController;
  onBack: () => void;
  onOpenSession: (id: string) => void;
}) {
  const [inspectedStep, setInspectedStep] = useState<string | null>(null);
  const [expandedTurns, setExpandedTurns] = useState<string[]>([]);
  const [allHistory, setAllHistory] = useState(false);
  const row = c.detail;
  if (!row) return null;
  const p = rowPresentation(row);
  const evidenceById = new Map(row.evidence.map((e) => [e.id, e]));
  const stepEvidence = (ids: string[]) =>
    ids.flatMap((id) => {
      const evidence = evidenceById.get(id);
      return evidence ? [evidence] : [];
    });
  const failedEvidence = new Set(
    row.evidence
      .filter((e) => e.exitCode !== undefined && e.exitCode !== 0)
      .map((e) => e.id),
  );
  const offPlanCalls = new Set(row.offPlan.flatMap((run) => run.callIds));
  const times = [
    ...row.asks.map((a) => Date.parse(a.at)),
    ...row.turns.map((t) => Date.parse(t.at)),
    Date.parse(row.lastActiveAt),
  ].filter(Number.isFinite);
  const start = Math.min(...times),
    end = Math.max(
      ...times,
      p.ended ? Date.parse(row.lastActiveAt) : Date.now(),
    ),
    span = Math.max(end - start, 1);
  const left = (at: string) => ((Date.parse(at) - start) / span) * 100;
  const evidenceBuckets = new Map<number, typeof row.evidence>();
  for (const evidence of row.evidence) {
    if (
      evidence.kind === "command" &&
      (evidence.exitCode === undefined || evidence.exitCode === 0)
    )
      continue;
    const bucket = Math.min(49, Math.max(0, Math.floor(left(evidence.at) / 2)));
    evidenceBuckets.set(bucket, [
      ...(evidenceBuckets.get(bucket) ?? []),
      evidence,
    ]);
  }
  const tickMinutes = span > 180 * 60000 ? 60 : 30;
  const tickInterval = tickMinutes * 60000;
  const ticks = [
    { at: start, left: 0, label: time(new Date(start).toISOString()) },
  ];
  for (
    let at = Math.ceil(start / tickInterval) * tickInterval;
    at <= end;
    at += tickInterval
  ) {
    const position = ((at - start) / span) * 100;
    if (position > 4 && position < 92)
      ticks.push({
        at,
        left: position,
        label: time(new Date(at).toISOString()),
      });
  }
  ticks.push({
    at: end,
    left: 100,
    label: p.ended ? time(row.lastActiveAt) : "现在",
  });
  const groups = groupRows(c.rows, "urgency");
  const filteredSteps = (r: LedgerDetail, turnId: string) =>
    r.trail.filter(
      (s) =>
        s.turnId === turnId && (!c.itemFilter || s.itemId === c.itemFilter),
    );
  return (
    <div className={`${styles.workspace} ${styles.agentWorkspace}`}>
      <aside className={styles.sidebar}>
        <div className={styles.agentSidebarHeader}>
          <button onClick={onBack}>Keepline</button>
          <span className={styles.subtle} style={{ fontSize: 12 }}>
            {time(new Date().toISOString())}
          </span>
          <button onClick={onBack} className={styles.cardMenuTrigger}>
            ← 总览
          </button>
        </div>
        <div style={{ padding: "0 20px 16px", fontSize: 15, lineHeight: 1.4 }}>
          {`${c.rows.filter((r) => rowPresentation(r).need).length} 件事需要你，${c.rows.filter((r) => !rowPresentation(r).need && !rowPresentation(r).ended && !rowPresentation(r).paused).length} 个 agent 在正常推进`}
        </div>
        {groups
          .filter((g) => g.rows.length)
          .map((g) => (
            <section key={g.key}>
              <div className={styles.agentGroupHeading}>
                <span>{g.label}</span>
                <span>{g.rows.length}</span>
              </div>
              {g.rows
                .filter(
                  (r, i) =>
                    g.key !== "ended" ||
                    allHistory ||
                    i < 4 ||
                    r.sessionId === row.sessionId,
                )
                .map((r) => {
                  const rp = rowPresentation(r);
                  return (
                    <button
                      key={r.sessionId}
                      onClick={() => c.setSelectedId(r.sessionId)}
                      className={`${styles.agentSession} ${r.sessionId === row.sessionId ? styles.agentSessionSelected : ""}`}
                    >
                      <div className={styles.spaceBetween}>
                        <StatusTag tag={rp.tag} bg={rp.tagBg} fg={rp.tagFg} />
                        <span className={styles.mono} style={{ fontSize: 12 }}>
                          <span title={rp.elapsed}>{rp.compactElapsed}</span>
                        </span>
                      </div>
                      <div
                        style={{
                          fontWeight: 600,
                          marginTop: 5,
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                      >
                        {r.title}
                      </div>
                      <div
                        className={styles.subtle}
                        style={{
                          fontSize: 12,
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                      >
                        {rp.agent} · {projectName(r.projectRoot)}
                      </div>
                      <div
                        className={`${styles.actions} ${styles.agentSidebarProgress}`}
                      >
                        <ProgressSegments items={rp.items} />
                        <span>
                          {r.progress.total
                            ? `${r.progress.done} / ${r.progress.total} 项有证据`
                            : "候选项 · 不计进度"}
                        </span>
                      </div>
                    </button>
                  );
                })}
              {g.key === "ended" && g.rows.length > 4 && (
                <button
                  className={styles.historyToggle}
                  aria-expanded={allHistory}
                  onClick={() => setAllHistory(!allHistory)}
                >
                  {allHistory
                    ? "收起历史会话 ↑"
                    : `查看全部 ${g.rows.length} 个已结束会话 ↓`}
                </button>
              )}
            </section>
          ))}
        <div className={styles.sidebarBottom}>
          <span className={styles.subtle}>
            J/K 切换 · Enter 看进度账 · O 跳到 agent · C 复制追问
          </span>
        </div>
      </aside>
      <main className={styles.agentPage}>
        <header className={styles.agentHeader}>
          <div className={styles.agentMeta}>
            <StatusTag tag={p.tag} bg={p.tagBg} fg={p.tagFg} />
            <span className={styles.subtle}>
              {`${p.agent} · ${row.projectRoot}`}
            </span>
          </div>
          <div className={styles.agentTitleRow}>
            <h1>{row.title}</h1>
            <div className={styles.actions}>
              <ActionButton
                primary
                onClick={() => onOpenSession(row.sessionId)}
              >
                跳到 agent
              </ActionButton>
              <ActionButton
                onClick={() => void c.perform(() => c.openPrompt(row))}
              >
                复制追问 (C)
              </ActionButton>
            </div>
          </div>
          <div className={styles.subtle} style={{ fontSize: 13 }}>
            会话跨度 <span className={styles.mono}>{p.elapsed}</span>
            {" · 最后动作 "}
            <span className={styles.mono}>{p.lastAgo}</span>
            {" · "}
            {row.turns.length}
            {" 轮 · 等你共 "}
            <span className={styles.mono} title="当前接口没有提供等待累计时长">
              未知
            </span>
          </div>
        </header>
        {c.error && (
          <div
            className={`${styles.errorBanner} ${styles.agentError}`}
            role="alert"
          >
            {c.error}
            <ActionButton onClick={() => void c.perform(c.load)}>
              重试
            </ActionButton>
          </div>
        )}
        {!row.available && (
          <div className={styles.empty}>
            {row.unavailableReason}。可查看会话状态或原始记录。
          </div>
        )}
        <section>
          <div className={styles.sectionHeading}>
            <h2 className={styles.agentSectionTitle}>时间线</h2>
            <span className={styles.timelineLegend}>
              <span>
                <i style={{ background: "#4F5BD5" }} />
                在做
              </span>
              <span>
                <i style={{ background: "#E6E8EC" }} />
                等你
              </span>
              <span>
                <i style={{ background: "#F59E0B" }} />
                计划外
              </span>
              <span>▼ 你发的话</span>
              <span>
                <i style={{ width: 2, background: "#15171C" }} />
                关键证据
              </span>
            </span>
          </div>
          <div className={styles.timeline}>
            {row.asks.map((ask) => (
              <button
                key={ask.id}
                className={styles.timelineAsk}
                style={{ left: `${left(ask.at)}%` }}
                title={ask.text}
                aria-label="你发的话"
                onClick={() =>
                  document
                    .getElementById(`ask-${ask.id}`)
                    ?.scrollIntoView({ block: "center" })
                }
              >
                ▼
              </button>
            ))}
            <div className={styles.timelineTrack}>
              {row.turns.map((turn) => (
                <span
                  key={turn.id}
                  className={styles.timelineSegment}
                  style={{
                    left: `${left(turn.at)}%`,
                    width: `${Math.max(0.2, Math.min((turn.durationMs / span) * 100, 100 - left(turn.at)))}%`,
                    background:
                      turn.phase === "aborted" ? "#F59E0B" : "#4F5BD5",
                  }}
                  title={`${time(turn.at)} · ${turn.phase}`}
                />
              ))}
              {row.offPlan.map((run) => (
                <span
                  key={run.id}
                  className={styles.timelineSegment}
                  style={{
                    left: `${left(run.at)}%`,
                    width: 2,
                    background: "#F59E0B",
                  }}
                />
              ))}
              {[...evidenceBuckets].map(([bucket, entries]) => (
                <span
                  key={bucket}
                  className={styles.timelineSegment}
                  style={{
                    left: `${bucket * 2}%`,
                    width: 2,
                    background: entries.some(
                      (e) => e.exitCode !== undefined && e.exitCode !== 0,
                    )
                      ? "#F59E0B"
                      : "#15171C",
                  }}
                  title={`${entries.length} 条关键证据 · ${entries.slice(0, 3).map(evidenceSummary).join("；")}${entries.length > 3 ? "…" : ""}`}
                />
              ))}
              <span
                className={styles.timelineSegment}
                style={{ right: 0, width: 2, background: "#15171C" }}
              />
            </div>
            {ticks.map((tick, i) => (
              <span
                key={tick.at}
                className={styles.timelineTick}
                style={{
                  left: `${tick.left}%`,
                  transform:
                    i === ticks.length - 1
                      ? "translateX(-100%)"
                      : i === 0
                        ? "none"
                        : "translateX(-50%)",
                }}
              >
                {tick.label}
              </span>
            ))}
          </div>
        </section>
        <section className={styles.agentSummary}>
          <span className={styles.subtle}>现在</span>
          <span style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span
              className={row.state === "running" ? styles.liveDot : undefined}
              style={{
                width: 8,
                height: 8,
                borderRadius: 4,
                background: row.state === "running" ? "#4F5BD5" : "#F59E0B",
                display: "inline-block",
                flex: "none",
              }}
            />
            <span>{p.now}</span>
            <span className={styles.subtle}>
              ·{" "}
              {row.items.find((item) => item.status === "doing")?.title ??
                p.lastAgo}
            </span>
          </span>
          <span className={styles.subtle}>最近证据</span>
          <span>
            <span className={styles.summaryEvidence} title={p.evidence}>
              {p.evidenceSummary}
            </span>{" "}
            <span className={styles.subtle}>{p.lastAgo}</span>
          </span>
          <span className={styles.subtle}>它说</span>
          <span className={styles.subtle}>
            {(row.activity?.lastMessage?.length ?? 0) > 180 ? (
              <details className={styles.messageDisclosure}>
                <summary>
                  {row.activity!.lastMessage!.slice(0, 180)}…{" "}
                  <span>展开汇报 ▾</span>
                </summary>
                <p>{row.activity?.lastMessage}</p>
              </details>
            ) : (
              <>“{row.activity?.lastMessage ?? "暂无汇报"}” </>
            )}
            <span style={{ fontSize: 12 }}>· 口头说法，不算证据</span>
          </span>
        </section>
        <div className={styles.agentSplit}>
          <section>
            <h2 className={styles.agentSectionTitle}>
              你发了什么 · 原话按时间
            </h2>
            {row.asks.map((ask) => (
              <div
                id={`ask-${ask.id}`}
                key={ask.id}
                className={styles.agentAsk}
              >
                <span
                  className={styles.mono}
                  style={{ fontSize: 12, paddingTop: 2 }}
                >
                  {time(ask.at)}
                </span>
                <div>
                  <p>{ask.text}</p>
                  <div
                    className={styles.subtle}
                    style={{ fontSize: 12, marginTop: 4 }}
                  >
                    {ask.kind === "question"
                      ? "问题 · 不计进度"
                      : ask.kind === "addition"
                        ? "追加要求"
                        : "最初要求"}
                  </div>
                </div>
              </div>
            ))}
          </section>
          <section id="ledger-requirements">
            <div className={styles.requirementsHeading}>
              <h2 className={styles.agentSectionTitle}>做到哪了</h2>
              <span>
                {`${row.progress.done} / ${row.progress.total} 项有证据`}
              </span>
              <ActionButton
                className={styles.editRequirements}
                onClick={() => c.setItemEditor(structuredClone(row.items))}
              >
                编辑要求
              </ActionButton>
              <span
                className={styles.subtle}
                style={{ fontSize: 12, flexBasis: "100%" }}
              >
                点一项，下面只看它的步骤
              </span>
            </div>
            <div className={styles.agentItems}>
              {row.items.map((item) => (
                <button
                  key={item.id}
                  className={styles.agentItem}
                  style={{
                    outline:
                      c.itemFilter === item.id ? "2px solid #15171C" : "none",
                  }}
                  onClick={() =>
                    c.setItemFilter((f) => (f === item.id ? null : item.id))
                  }
                >
                  <span
                    className={styles.square}
                    style={{
                      width: 12,
                      height: 12,
                      background: itemColors[item.status],
                      borderRadius:
                        item.status === "done"
                          ? 0
                          : item.status === "doing"
                            ? "50%"
                            : 2,
                    }}
                  />
                  <span>
                    {item.title}
                    <span
                      className={styles.subtle}
                      style={{ fontSize: 12, marginLeft: 8 }}
                    >
                      {sourceText[item.source]}
                    </span>
                  </span>
                  <span
                    className={styles.subtle}
                    style={{
                      fontSize: 12,
                      color:
                        item.status === "unverified"
                          ? "#B45309"
                          : item.status === "done"
                            ? "#4F5BD5"
                            : "#6B7280",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {item.status === "todo" ? "□" : "■"}{" "}
                    {requirementStatus(item)}
                  </span>
                </button>
              ))}
            </div>
            {row.items.some((i) => i.source === "fallback") && (
              <ActionButton
                disabled={c.busy}
                onClick={() =>
                  void c.perform(() =>
                    c.mutateDetail("items", { items: row.items }, "PUT"),
                  )
                }
              >
                确认这些要求
              </ActionButton>
            )}
            {c.settings.judge.enabled && (
              <ActionButton
                disabled={c.busy}
                onClick={() =>
                  void c.perform(() => c.mutateDetail("redecompose", {}))
                }
              >
                重新识别要求
              </ActionButton>
            )}
            {row.items.some((i) => i.constraints.length) && (
              <>
                <div
                  className={styles.subtle}
                  style={{ fontSize: 12, marginTop: 14 }}
                >
                  约束
                </div>
                {row.items
                  .flatMap((i) => i.constraints)
                  .map((cn, i) => (
                    <div key={i} style={{ fontSize: 13, padding: "4px 0" }}>
                      {cn.kind === "path_forbidden"
                        ? "禁止修改"
                        : cn.kind === "preserve_text"
                          ? "保留文字"
                          : "不改公共接口"}{" "}
                      {cn.value}
                    </div>
                  ))}
              </>
            )}
          </section>
        </div>
        <section id="ledger-current-step">
          <div className={styles.sectionHeading}>
            <h2 className={styles.agentSectionTitle}>它实际做了什么 · 按轮</h2>
            {c.itemFilter && (
              <ActionButton onClick={() => c.setItemFilter(null)}>
                只看：{row.items.find((i) => i.id === c.itemFilter)?.title} ×
              </ActionButton>
            )}
            <span
              className={styles.subtle}
              style={{ marginLeft: "auto", fontSize: 12 }}
            >
              默认显示最近关键步骤，全部调用可展开
            </span>
          </div>
          <div className={styles.agentTrail}>
            {row.turns.map((turn, i) => {
              const steps = filteredSteps(row, turn.id);
              const turnKey = `${row.sessionId}:${turn.id}`;
              const expanded = expandedTurns.includes(turnKey);
              const important = steps.filter(
                (step) =>
                  keyStep(step, stepEvidence(step.evidenceIds)) ||
                  offPlanCalls.has(step.callId),
              );
              const recent = new Set(
                important.slice(-8).map((step) => step.callId),
              );
              const visible =
                expanded || c.itemFilter
                  ? steps
                  : important.filter(
                      (step) =>
                        recent.has(step.callId) ||
                        step.violations.length ||
                        offPlanCalls.has(step.callId) ||
                        step.evidenceIds.some((id) => failedEvidence.has(id)),
                    );
              return (
                <div key={turn.id}>
                  <div className={styles.turnHeading}>
                    <span
                      style={{ color: "#15171C", fontWeight: 600 }}
                    >{`第 ${i + 1} 轮`}</span>
                    <span
                      className={styles.mono}
                    >{`${time(turn.at)} – ${turn.phase === "started" ? "现在" : time(new Date(Date.parse(turn.at) + turn.durationMs).toISOString())}`}</span>
                    <span>· {Math.round(turn.durationMs / 60000)} 分钟</span>
                    <span>· 只读 ×{turn.readOnlyCount}</span>
                    <span>· {steps.length} 条记录</span>
                    {steps.length > visible.length || expanded ? (
                      <button
                        className={styles.trailToggle}
                        aria-expanded={expanded}
                        onClick={() =>
                          setExpandedTurns((keys) =>
                            expanded
                              ? keys.filter((key) => key !== turnKey)
                              : [...keys, turnKey],
                          )
                        }
                      >
                        {expanded
                          ? "只看关键步骤 ↑"
                          : `展开全部 ${steps.length} 条记录 ↓`}
                      </button>
                    ) : null}
                  </div>
                  {!visible.length && (
                    <p className={styles.trailEmpty}>
                      暂无匹配的关键步骤，可展开全部执行记录。
                    </p>
                  )}
                  {visible.map((step) => (
                    <div key={step.callId} className={styles.stepRow}>
                      <span className={styles.mono} style={{ fontSize: 12 }}>
                        {time(step.at)}
                      </span>
                      <span
                        className={styles.mono}
                        style={{ fontSize: 12 }}
                        title={step.name}
                      >
                        {step.name}
                      </span>
                      <button
                        className={styles.stepSummary}
                        title={step.summary}
                        aria-label={step.summary}
                        aria-expanded={inspectedStep === step.callId}
                        onClick={() =>
                          setInspectedStep(
                            inspectedStep === step.callId ? null : step.callId,
                          )
                        }
                      >
                        {step.summary.includes('{"')
                          ? "执行调用 · 展开原始记录"
                          : step.summary}
                        {stepEvidence(step.evidenceIds).map((e) => (
                          <span
                            key={e.id}
                            className={styles.evidence}
                            style={{ marginLeft: 10 }}
                          >
                            {evidenceSummary(e)}
                          </span>
                        ))}
                      </button>
                      <span
                        className={styles.stepTag}
                        style={
                          step.itemId
                            ? { background: "#EEF0FD", color: "#4F5BD5" }
                            : undefined
                        }
                      >
                        {step.itemId
                          ? `第 ${row.items.findIndex((i) => i.id === step.itemId) + 1} 项`
                          : step.acceptedOffPlan
                            ? "你已接受的偏离"
                            : "未归属"}
                      </span>
                      <div
                        className={styles.stepInspector}
                        hidden={inspectedStep !== step.callId}
                      >
                        <strong>
                          证据与归属 · {step.evidenceIds.length} 条
                        </strong>
                        <details className={styles.rawStep}>
                          <summary>原始执行记录</summary>
                          <pre>{step.summary}</pre>
                        </details>
                        {stepEvidence(step.evidenceIds).map((e) => (
                          <pre key={e.id}>
                            {e.kind}: {e.value}{" "}
                            {e.exitCode !== undefined
                              ? `· exit ${e.exitCode}`
                              : ""}
                          </pre>
                        ))}
                        {!!step.violations.length && (
                          <p style={{ color: "#B45309" }}>
                            {step.violations.join("；")}
                          </p>
                        )}
                        <div className={styles.actions}>
                          <select
                            aria-label="步骤归属"
                            value={step.itemId ?? ""}
                            onChange={(e) =>
                              void c.perform(() =>
                                c.mutateDetail("corrections", {
                                  callId: step.callId,
                                  itemId: e.target.value || undefined,
                                }),
                              )
                            }
                          >
                            <option value="">未归属</option>
                            {row.items
                              .filter(confirmedRequirement)
                              .map((item) => (
                                <option key={item.id} value={item.id}>
                                  {item.title}
                                </option>
                              ))}
                          </select>
                          <ActionButton
                            disabled={!step.itemId || c.busy}
                            onClick={() => {
                              const paths = window.prompt(
                                "相似步骤的路径（例如 src/ledger/**）",
                              );
                              if (paths?.trim() && step.itemId)
                                void c.perform(() =>
                                  c.mutateDetail("corrections", {
                                    callId: step.callId,
                                    itemId: step.itemId,
                                    rule: {
                                      itemId: step.itemId,
                                      matcher: {
                                        paths: [paths.trim()],
                                        commands: [],
                                      },
                                    },
                                  }),
                                );
                            }}
                          >
                            后续相似步骤也这样归属
                          </ActionButton>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              );
            })}
            {!!row.subagents?.length && (
              <div className={styles.empty}>
                {row.subagents.map((child) => (
                  <button
                    key={child.sessionId}
                    className={styles.todoTitle}
                    onClick={() => c.setSelectedId(child.sessionId)}
                  >
                    {child.title} · {child.activity?.action}
                  </button>
                ))}
              </div>
            )}
          </div>
        </section>
        {row.offPlan.map((run, i) => (
          <section
            id={i === 0 ? "ledger-off-plan" : undefined}
            key={run.id}
            className={styles.agentSummary}
          >
            <strong style={{ gridColumn: "1/-1" }}>
              可能偏离 · {run.callIds.length} 步
            </strong>
            <div className={styles.actions} style={{ gridColumn: "1/-1" }}>
              <ActionButton
                onClick={() =>
                  void c.perform(() =>
                    c.mutateDetail("corrections", {
                      callIds: run.callIds,
                      acceptOffPlan: true,
                    }),
                  )
                }
              >
                接受这段操作，不再提醒
              </ActionButton>
              <ActionButton
                onClick={() => void c.perform(() => c.openPrompt(row, run.id))}
              >
                生成纠正提示
              </ActionButton>
            </div>
          </section>
        ))}
        <section>
          <h2 className={styles.agentSectionTitle}>它说的 vs 证据显示的</h2>
          <div className={styles.agentTrail}>
            {row.claims.map((claim, i) => (
              <div key={i} className={styles.claimRow}>
                <span className={styles.subtle}>“{claim.text}”</span>
                <span>
                  {claim.evidenceIds.length
                    ? row.evidence
                        .filter((e) => claim.evidenceIds.includes(e.id))
                        .map((e) => (
                          <span key={e.id} className={styles.evidence}>
                            {e.value}{" "}
                          </span>
                        ))
                    : "暂无匹配证据"}
                </span>
                {claim.evidenceIds.length ? (
                  <span style={{ fontSize: 12, color: "#4F5BD5" }}>
                    ■ 有匹配证据
                  </span>
                ) : (
                  <ActionButton
                    onClick={() => void c.perform(() => c.openPrompt(row))}
                  >
                    要求补证据
                  </ActionButton>
                )}
              </div>
            ))}
          </div>
        </section>
        <section className={styles.agentSummary}>
          <strong style={{ gridColumn: "1/-1" }}>关联目标</strong>
          <div className={styles.actions} style={{ gridColumn: "1/-1" }}>
            <span>{c.itemPath(row)}</span>
            <select
              aria-label="关联待办"
              value={row.workItemId ?? ""}
              onChange={(e) =>
                void c.perform(() =>
                  c.mutateDetail("attribution", {
                    workItemId: e.target.value || null,
                  }),
                )
              }
            >
              <option value="">不关联目标</option>
              {c.todos.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.title}
                </option>
              ))}
            </select>
            {!row.workItemId &&
              row.attribution?.map((a) => (
                <ActionButton
                  key={a.workItemId}
                  onClick={() =>
                    void c.perform(() =>
                      c.mutateDetail("attribution", {
                        workItemId: a.workItemId,
                      }),
                    )
                  }
                >
                  {a.title} · {a.reasons.join("；")}
                </ActionButton>
              ))}
          </div>
        </section>
        {!!row.importSuggestions?.length && (
          <section className={styles.actions}>
            {row.importSuggestions.map((previous) => (
              <ActionButton
                key={previous.sessionId}
                onClick={() =>
                  void c.perform(() =>
                    c.mutateDetail("import-requirements", {
                      fromSessionId: previous.sessionId,
                    }),
                  )
                }
              >
                导入要求：{previous.title}
              </ActionButton>
            ))}
          </section>
        )}
        {!!row.followUpSuggestions?.length && (
          <section>
            <h2 className={styles.agentSectionTitle}>建议后续待办</h2>
            {row.followUpSuggestions.map((text) => (
              <div key={text} className={styles.spaceBetween}>
                <span>{text}</span>
                <ActionButton
                  onClick={() =>
                    c.setEditing({
                      title: text,
                      level: "task",
                      parentId:
                        c.todos.find((t) => t.id === row.workItemId)
                          ?.parentId ?? "",
                      outcome: "",
                      checklist: "",
                      projectRoot: row.projectRoot,
                    })
                  }
                >
                  预览新待办
                </ActionButton>
              </div>
            ))}
          </section>
        )}
        {((row.state === "review" && row.progress.total > 0) ||
          (row.state === "ended" &&
            row.progress.total > 0 &&
            row.turns.find((t) => t.id === row.turnId)?.phase ===
              "completed")) && (
          <section>
            <h2 className={styles.agentSectionTitle}>核对这一轮</h2>
            <div className={styles.actions}>
              <ActionButton
                primary
                disabled={c.busy || row.progress.done !== row.progress.total}
                onClick={() =>
                  void c.perform(() =>
                    c.mutateDetail("acceptances", { decision: "accepted" }),
                  )
                }
              >
                验收通过
              </ActionButton>
              <ActionButton
                onClick={() => {
                  const reason = window.prompt("为什么放弃剩余要求？");
                  if (reason?.trim())
                    void c.perform(() =>
                      c.mutateDetail("acceptances", {
                        decision: "accepted_with_gaps",
                        reason,
                        droppedItemIds: row.items
                          .filter(
                            (i) => i.status !== "done" || !i.evidenceIds.length,
                          )
                          .map((i) => i.id),
                      }),
                    );
                }}
              >
                接受并放弃剩余项
              </ActionButton>
              <ActionButton
                onClick={() =>
                  void c.perform(async () => {
                    await c.mutateDetail("acceptances", {
                      decision: "follow_up",
                    });
                    await c.openPrompt(row);
                  })
                }
              >
                生成后续提示
              </ActionButton>
            </div>
          </section>
        )}
        {!!row.acceptances.length && (
          <section>
            {row.acceptances.map((a) => (
              <div key={a.turnId} className={styles.subtle}>
                {a.decision === "accepted"
                  ? "已验收"
                  : a.decision === "accepted_with_gaps"
                    ? "接受并放弃剩余项"
                    : "继续跟进"}{" "}
                · {a.reason} · {new Date(a.at).toLocaleString()}
              </div>
            ))}
          </section>
        )}
      </main>
    </div>
  );
}
