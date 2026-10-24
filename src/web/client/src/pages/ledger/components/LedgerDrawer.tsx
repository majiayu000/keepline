import { useEffect, useRef } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import type { LedgerDetail } from "../../../../../../domain/ledger/types";
import type { RecoveryInfo } from "@/types";
import {
  rowPresentation,
  projectName,
  time,
  requirementStatus,
  stateText,
  itemColors,
} from "../presentation";
import { StatusTag, SubtaskTag, ActionButton } from "./LedgerPrimitives";
import styles from "../Workspace.module.css";
import { LedgerRecap } from "./LedgerRecap";
import { LedgerAskText } from "./LedgerAskText";

const markdownComponents: Components = {
  a: ({ href, children }) => (
    <a href={href} target="_blank" rel="noopener noreferrer">{children}</a>
  ),
  table: ({ children }) => (
    <div className={styles.markdownTable}><table>{children}</table></div>
  ),
};

export function LedgerDrawer({
  row,
  modal,
  onClose,
  onOpenSession,
  onFullDetail,
  onSelectSession,
  recovery,
  onCopyRecovery,
}: {
  row: LedgerDetail;
  modal: boolean;
  onClose: () => void;
  onOpenSession: () => void;
  onFullDetail: () => void;
  onSelectSession: (sessionId: string) => void;
  recovery: RecoveryInfo | null;
  onCopyRecovery: () => void;
}) {
  const p = rowPresentation(row);
  const message = row.activity?.lastMessage;
  const report = <ReactMarkdown
    className={`${styles.markdown} ${styles.drawerReport}`}
    remarkPlugins={[remarkGfm]}
    components={markdownComponents}
    skipHtml
  >{message ?? "尚无 Agent 回复记录"}</ReactMarkdown>;
  const ref = useRef<HTMLElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    ref.current
      ?.querySelector<HTMLButtonElement>('button[aria-label="关闭"]')
      ?.focus();
    return () => { if (previous?.isConnected) previous.focus(); };
  }, [row.sessionId]);
  return (
    <aside
      ref={ref}
      className={styles.drawer}
      role="dialog"
      aria-modal={modal}
      aria-label={`会话详情：${row.title}`}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <header className={styles.drawerHeader}>
        <div className={styles.drawerHeading}>
          <div className={styles.drawerEyebrow}>
            <StatusTag tag={p.tag} bg={p.tagBg} fg={p.tagFg} />
            {(row.isSubAgent || row.parentSessionId) && <SubtaskTag />}
            <span>{p.agent} · {projectName(row.projectRoot)} · 会话跨度 {p.elapsed}</span>
          </div>
          <h2>{row.title}</h2>
        </div>
        <button className={styles.close} aria-label="关闭" onClick={onClose}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden="true">
            <path d="m6 6 12 12M18 6 6 18" />
          </svg>
        </button>
      </header>
      <div className={styles.drawerBody}>
        {!row.available && <p role="status">{row.unavailableReason ?? "当前无法完整读取执行记录"}；以下仅展示已取得的记录。</p>}
        {row.parentSessionId && (
          <div className={styles.drawerSection}>
            <ActionButton onClick={() => onSelectSession(row.parentSessionId!)}>查看父会话</ActionButton>
          </div>
        )}
        {!!row.subagents?.length && (
          <section className={styles.drawerSection} aria-label="子任务列表">
            <SubtaskTag count={row.subagents.length} />
            {row.subagents.map(child => (
              <button key={child.sessionId} className={styles.subtaskRow}
                aria-label={`查看子任务 ${child.title}`} onClick={() => onSelectSession(child.sessionId)}>
                <SubtaskTag />
                <span><strong>{child.title}</strong><small>{child.activity?.action ?? "暂无执行记录"}</small></span>
                <span>{stateText[child.state]}</span>
              </button>
            ))}
          </section>
        )}
        <div className={styles.drawerActivity}>
          <span className={styles.subtle}>现在</span>
          <span className={styles.drawerFactValue}>
            {p.now} · {p.lastAgo}
          </span>
          <span className={styles.subtle}>最新回复</span>
          {message && message.length > 360 ? <details className={`${styles.messageDisclosure} ${styles.drawerReport}`}>
            <summary>{message.slice(0, 180)}… <span>展开完整回复</span></summary>
            {report}
          </details> : report}
          <span className={styles.subtle}>上下文</span>
          <span style={{ color: p.context?.fg ?? "#6B7280" }}>
            {p.context?.detail ?? "未知"}
          </span>
        </div>
        <LedgerRecap row={row} />
        <details className={styles.drawerOptional}>
          <summary>你发的消息 · 最新在前</summary>
          {[...row.asks].reverse().map((ask) => (
            <div key={ask.id} className={styles.askRow}>
              <span className={styles.mono}>{time(ask.at)}</span>
              <LedgerAskText text={ask.text} />
            </div>
          ))}
        </details>
        {p.items.length > 0 && <details className={styles.drawerOptional}>
          <summary>
            要求与验收 ·{" "}
            <span>{`${row.progress.done} / ${row.progress.total}`}</span>
          </summary>
          {p.items.map((item) => (
            <div key={item.id} className={styles.requirementRow}>
              <span
                className={styles.square}
                style={{ background: itemColors[item.status] }}
              />
              <span>{item.title}</span>
              <span
                style={{
                  color:
                    item.status === "unverified"
                      ? "#B45309"
                      : item.status === "done"
                        ? "#4F5BD5"
                        : "#6B7280",
                  whiteSpace: "nowrap",
                }}
              >
                {item.status === "todo" ? "□" : "■"} {requirementStatus(item)}
              </span>
            </div>
          ))}
        </details>}
        {row.items.some((i) => i.constraints.length > 0) && (
          <details className={styles.drawerOptional}>
            <summary>约束 · 规则检查，不计入进度</summary>
            {row.items
              .flatMap((i) => i.constraints)
              .map((cn, i) => (
                <div key={i} className={styles.requirementRow}>
                  <span
                    className={styles.square}
                    style={{
                      border: "1px solid #6B7280",
                      boxSizing: "border-box",
                    }}
                  />
                  <span>
                    {cn.kind === "path_forbidden"
                      ? "禁止修改"
                      : cn.kind === "preserve_text"
                        ? "保留文字"
                        : "不改公共接口"}{" "}
                    {cn.value}
                  </span>
                  <span className={styles.subtle}>见执行轨迹</span>
                </div>
              ))}
          </details>
        )}
        {recovery?.canRecover && recovery.command && (
          <div className={styles.drawerSection}>
            <div>进程已不在，会话文件还在 · 可恢复</div>
            <pre className={styles.evidence} style={{ whiteSpace: "pre-wrap" }}>
              {recovery.command}
            </pre>
            <ActionButton onClick={onCopyRecovery}>复制恢复命令</ActionButton>
          </div>
        )}
      </div>
      <div className={styles.drawerActions}>
        <ActionButton primary onClick={onOpenSession}>
          查看会话记录
        </ActionButton>
        <ActionButton onClick={onFullDetail}>完整进度账 →</ActionButton>
      </div>
    </aside>
  );
}
